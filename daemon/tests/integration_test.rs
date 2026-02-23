use clawd::{
    account::AccountRegistry, config::DaemonConfig, ipc::event::EventBroadcaster,
    repo::RepoRegistry, scheduler, session::SessionManager, storage::Storage, tasks::TaskStorage,
    telemetry, update, worktree, AppContext,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
/// Integration tests for the clawd JSON-RPC server.
/// Spins up a real daemon on a free port and tests all RPC methods.
use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::sync::Arc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Start a daemon on a random port and return the WebSocket URL.
async fn start_test_daemon() -> (String, Arc<AppContext>) {
    let data_dir = tempfile::tempdir().unwrap().keep();
    let port = get_free_port();

    let config = Arc::new(DaemonConfig::new(
        Some(port),
        Some(data_dir.clone()),
        Some("warn".to_string()),
        None,
    ));
    let storage = Arc::new(Storage::new(&data_dir).await.unwrap());
    let storage_pool = storage.pool();
    let broadcaster = Arc::new(EventBroadcaster::new());
    let repo_registry = Arc::new(RepoRegistry::new(broadcaster.clone()));
    let session_manager = Arc::new(SessionManager::new(
        storage.clone(),
        broadcaster.clone(),
        data_dir.clone(),
    ));

    let config_arc = Arc::clone(&config);
    let account_registry = Arc::new(AccountRegistry::new(storage.clone(), broadcaster.clone()));
    let updater = Arc::new(update::spawn(config_arc.clone(), broadcaster.clone()));
    let account_pool = Arc::new(scheduler::accounts::AccountPool::new());
    let rate_limit_tracker = Arc::new(scheduler::rate_limits::RateLimitTracker::new());
    let fallback_engine = Arc::new(scheduler::fallback::FallbackEngine::new(
        Arc::clone(&account_pool),
        Arc::clone(&rate_limit_tracker),
    ));
    let ctx = Arc::new(AppContext {
        config,
        storage,
        broadcaster,
        repo_registry,
        session_manager,
        daemon_id: "test-daemon-id".to_string(),
        license: Arc::new(tokio::sync::RwLock::new(clawd::license::LicenseInfo::free())),
        telemetry: Arc::new(telemetry::spawn(
            config_arc,
            "test-daemon-id".to_string(),
            "free".to_string(),
        )),
        account_registry,
        updater,
        started_at: std::time::Instant::now(),
        auth_token: String::new(),
        task_storage: Arc::new(TaskStorage::new(storage_pool)),
        worktree_manager: Arc::new(worktree::WorktreeManager::new(&data_dir)),
        account_pool,
        rate_limit_tracker,
        fallback_engine,
        scheduler_queue: Arc::new(scheduler::queue::SchedulerQueue::new()),
        orchestrator: Arc::new(clawd::agents::orchestrator::Orchestrator::new()),
    });

    let ctx_server = ctx.clone();
    tokio::spawn(async move {
        clawd::ipc::run(ctx_server).await.ok();
    });

    // Give server a moment to bind
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let url = format!("ws://127.0.0.1:{}", ctx.config.port);
    (url, ctx)
}

fn get_free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

async fn ws_rpc(url: &str, method: &str, params: Value) -> Value {
    let (mut ws, _) = connect_async(url).await.expect("ws connect failed");

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    });
    ws.send(Message::Text(serde_json::to_string(&request).unwrap()))
        .await
        .unwrap();

    // Read messages until we get the response (skip notifications)
    loop {
        let msg = ws.next().await.unwrap().unwrap();
        if let Message::Text(text) = msg {
            let v: Value = serde_json::from_str(&text).unwrap();
            if v.get("id").is_some() {
                return v;
            }
        }
    }
}

#[tokio::test]
async fn test_daemon_ping() {
    let (url, _ctx) = start_test_daemon().await;
    let resp = ws_rpc(&url, "daemon.ping", json!({})).await;
    assert_eq!(resp["result"]["pong"], true);
}

#[tokio::test]
async fn test_daemon_status() {
    let (url, _ctx) = start_test_daemon().await;
    let resp = ws_rpc(&url, "daemon.status", json!({})).await;
    let result = &resp["result"];
    assert!(result["version"].is_string());
    assert!(result["uptime"].is_number());
    assert_eq!(result["activeSessions"], 0);
    assert_eq!(result["watchedRepos"], 0);
}

#[tokio::test]
async fn test_method_not_found() {
    let (url, _ctx) = start_test_daemon().await;
    let resp = ws_rpc(&url, "no.such.method", json!({})).await;
    assert_eq!(resp["error"]["code"], -32601);
}

#[tokio::test]
async fn test_session_create_list_get_delete() {
    let (url, _ctx) = start_test_daemon().await;
    let tmp_repo = tempfile::tempdir().unwrap();

    // Create session
    let resp = ws_rpc(
        &url,
        "session.create",
        json!({
            "provider": "claude",
            "repoPath": tmp_repo.path().to_str().unwrap(),
            "title": "Test Session"
        }),
    )
    .await;
    assert!(resp.get("error").is_none(), "create error: {:?}", resp);
    let session = &resp["result"];
    let session_id = session["id"].as_str().unwrap().to_string();
    assert_eq!(session["provider"], "claude");
    assert_eq!(session["title"], "Test Session");
    assert_eq!(session["status"], "idle");

    // List sessions
    let list_resp = ws_rpc(&url, "session.list", json!({})).await;
    let sessions = list_resp["result"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], session_id);

    // Get session
    let get_resp = ws_rpc(&url, "session.get", json!({ "sessionId": session_id })).await;
    assert_eq!(get_resp["result"]["id"], session_id);

    // Delete session
    let del_resp = ws_rpc(&url, "session.delete", json!({ "sessionId": session_id })).await;
    assert!(del_resp.get("error").is_none());

    // List should be empty
    let list_again = ws_rpc(&url, "session.list", json!({})).await;
    let sessions = list_again["result"].as_array().unwrap();
    assert_eq!(sessions.len(), 0);
}

#[tokio::test]
async fn test_session_not_found() {
    let (url, _ctx) = start_test_daemon().await;
    let resp = ws_rpc(
        &url,
        "session.get",
        json!({ "sessionId": "nonexistent-id" }),
    )
    .await;
    assert_eq!(resp["error"]["code"], -32001);
}

#[tokio::test]
async fn test_repo_not_a_git_repo() {
    let (url, _ctx) = start_test_daemon().await;
    let tmp = tempfile::tempdir().unwrap();
    let resp = ws_rpc(
        &url,
        "repo.open",
        json!({ "repoPath": tmp.path().to_str().unwrap() }),
    )
    .await;
    // Should return an error since it's not a git repo
    assert!(
        resp.get("error").is_some(),
        "expected error for non-git dir"
    );
}

#[tokio::test]
async fn test_get_messages_empty() {
    let (url, _ctx) = start_test_daemon().await;
    let tmp_repo = tempfile::tempdir().unwrap();

    let create_resp = ws_rpc(
        &url,
        "session.create",
        json!({
            "provider": "claude",
            "repoPath": tmp_repo.path().to_str().unwrap(),
            "title": "Msg Test"
        }),
    )
    .await;
    let session_id = create_resp["result"]["id"].as_str().unwrap();

    let msgs = ws_rpc(
        &url,
        "session.getMessages",
        json!({ "sessionId": session_id }),
    )
    .await;
    assert!(msgs["result"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_health_endpoint() {
    let (_url, ctx) = start_test_daemon().await;
    let port = ctx.config.port;

    // Give the server a moment to be ready
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // Use a blocking TCP connection in a spawn_blocking to avoid mixing sync I/O
    let result = tokio::task::spawn_blocking(move || {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))?;
        stream.write_all(b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n")?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        Ok::<String, std::io::Error>(response)
    })
    .await
    .unwrap()
    .expect("TCP connect failed");

    // Extract the JSON body (after the blank line separating headers from body)
    let body = result.split("\r\n\r\n").nth(1).unwrap_or(&result);
    let json: serde_json::Value = serde_json::from_str(body).expect("health body is not JSON");

    assert_eq!(json["status"], "ok");
    assert!(json["version"].is_string());
    assert!(json["uptime"].is_number());
    assert!(json["activeSessions"].is_number());
    assert!(json["port"].is_number());
}
