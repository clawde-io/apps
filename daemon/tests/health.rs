//! Unit tests for the HTTP health endpoint.
//! Spins up the IPC server on a random port and sends an HTTP GET /health request.

use clawd::{
    account::AccountRegistry,
    agents::orchestrator::Orchestrator,
    config::DaemonConfig,
    intelligence::token_tracker::TokenTracker,
    ipc::event::EventBroadcaster,
    license::LicenseInfo,
    repo::RepoRegistry,
    scheduler::{
        accounts::AccountPool, fallback::FallbackEngine, queue::SchedulerQueue,
        rate_limits::RateLimitTracker,
    },
    session::SessionManager,
    storage::Storage,
    tasks::TaskStorage,
    telemetry, update,
    worktree::WorktreeManager,
    AppContext,
};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Find a free local port by binding to port 0.
fn find_free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// Build a minimal AppContext on a random port for testing.
async fn make_test_ctx(dir: &TempDir, port: u16) -> Arc<AppContext> {
    let data_dir = dir.path().to_path_buf();
    let config = Arc::new(DaemonConfig::new(
        Some(port),
        Some(data_dir.clone()),
        Some("error".to_string()),
        None,
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
    let account_registry = Arc::new(AccountRegistry::new(storage.clone(), broadcaster.clone()));
    let updater = Arc::new(update::spawn(config.clone(), broadcaster.clone()));
    let account_pool = Arc::new(AccountPool::new());
    let rate_limit_tracker = Arc::new(RateLimitTracker::new());
    let fallback_engine = Arc::new(FallbackEngine::new(
        Arc::clone(&account_pool),
        Arc::clone(&rate_limit_tracker),
    ));

    let token_tracker = TokenTracker::new(storage.clone());
    Arc::new(AppContext {
        config: config.clone(),
        storage,
        broadcaster,
        repo_registry,
        session_manager,
        daemon_id: "test-daemon-id".to_string(),
        license: Arc::new(tokio::sync::RwLock::new(LicenseInfo::free())),
        telemetry: Arc::new(telemetry::spawn(
            config,
            "test-daemon-id".to_string(),
            "free".to_string(),
        )),
        account_registry,
        updater,
        started_at: std::time::Instant::now(),
        auth_token: String::new(),
        task_storage: Arc::new(TaskStorage::new(storage_pool)),
        worktree_manager: Arc::new(WorktreeManager::new(&data_dir)),
        account_pool,
        rate_limit_tracker,
        fallback_engine,
        scheduler_queue: Arc::new(SchedulerQueue::new()),
        orchestrator: Arc::new(Orchestrator::new()),
        token_tracker,
        metrics: Arc::new(clawd::metrics::DaemonMetrics::new()),
        version_watcher: Arc::new(clawd::doctor::version_watcher::VersionWatcher::new(
            Arc::new(clawd::ipc::event::EventBroadcaster::new()),
        )),
        ide_bridge: clawd::ide::new_shared_bridge(),
    })
}

#[tokio::test]
async fn test_health_endpoint_response_fields() {
    let dir = TempDir::new().unwrap();
    let port = find_free_port();
    let ctx = make_test_ctx(&dir, port).await;

    // Start the IPC server in the background
    let ctx_clone = ctx.clone();
    tokio::spawn(async move {
        let _ = clawd::ipc::run(ctx_clone).await;
    });

    // Give the server a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Send HTTP GET /health request
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let request = "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    stream.write_all(request.as_bytes()).await.unwrap();

    // Read response
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf);

    // Split headers from body
    let body_start = response
        .find("\r\n\r\n")
        .map(|i| i + 4)
        .or_else(|| response.find("\n\n").map(|i| i + 2))
        .expect("no body in response");
    let body = &response[body_start..];

    // Parse as JSON
    let json: serde_json::Value = serde_json::from_str(body).expect("body is not valid JSON");

    // Assert all required fields
    assert_eq!(json["status"], "ok", "status should be 'ok'");
    assert!(json["version"].is_string(), "version should be a string");
    assert!(json["uptime"].is_number(), "uptime should be a number");
    assert!(
        json["activeSessions"].is_number(),
        "activeSessions should be a number"
    );
    assert!(json["port"].is_number(), "port should be a number");

    // Assert version matches CARGO_PKG_VERSION
    assert_eq!(
        json["version"].as_str().unwrap(),
        env!("CARGO_PKG_VERSION"),
        "version should match CARGO_PKG_VERSION"
    );

    // Assert port matches the configured port
    assert_eq!(
        json["port"].as_u64().unwrap(),
        port as u64,
        "port in response should match configured port"
    );

    // Assert no sensitive fields
    assert!(
        json.get("auth_token").is_none(),
        "response must not expose auth_token"
    );
    assert!(
        json.get("data_dir").is_none(),
        "response must not expose data_dir"
    );
}

#[tokio::test]
async fn test_health_endpoint_returns_200() {
    let dir = TempDir::new().unwrap();
    let port = find_free_port();
    let ctx = make_test_ctx(&dir, port).await;

    let ctx_clone = ctx.clone();
    tokio::spawn(async move {
        let _ = clawd::ipc::run(ctx_clone).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf);

    // First line should be HTTP 200
    let first_line = response.lines().next().unwrap_or("");
    assert!(
        first_line.contains("200"),
        "expected HTTP 200, got: {first_line}"
    );
    assert!(
        response.contains("Content-Type: application/json"),
        "expected JSON content type"
    );
}
