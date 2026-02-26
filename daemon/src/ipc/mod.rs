pub mod auth;
pub mod event;
pub mod handlers;

use crate::AppContext;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_tungstenite::{
    accept_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
};
use tracing::{debug, error, info, trace, warn};

// ─── Rate limiting ──────────────────────────────────────────────────────────

/// Per-IP connection rate tracker.
struct ConnectionRateLimiter {
    /// Map of IP -> list of connection timestamps within the last minute.
    connections: HashMap<IpAddr, Vec<Instant>>,
}

impl ConnectionRateLimiter {
    fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    /// Returns `true` if the connection should be allowed.
    ///
    /// Localhost connections (`127.0.0.1`, `::1`) are always allowed — no rate limiting
    /// for the local daemon process or Flutter desktop app.
    fn check_and_record(&mut self, ip: IpAddr, limit: usize) -> bool {
        if ip.is_loopback() {
            return true;
        }
        let now = Instant::now();
        let one_min_ago = now - std::time::Duration::from_secs(60);

        let timestamps = self.connections.entry(ip).or_default();
        timestamps.retain(|t| *t > one_min_ago);

        if timestamps.len() >= limit {
            return false;
        }
        timestamps.push(now);
        true
    }
}

/// Per-connection RPC rate tracker using a tumbling window (resets each minute).
struct RpcRateLimiter {
    count: u32,
    max_per_min: u32,
    window_start: Instant,
    /// Loopback connections are never rate-limited.
    is_loopback: bool,
}

impl RpcRateLimiter {
    fn new(max_per_min: u32, is_loopback: bool) -> Self {
        Self {
            count: 0,
            max_per_min,
            window_start: Instant::now(),
            is_loopback,
        }
    }

    /// Returns `true` if the request should be allowed.
    fn check(&mut self) -> bool {
        if self.is_loopback {
            return true;
        }
        let now = Instant::now();
        if now.duration_since(self.window_start).as_secs() >= 60 {
            self.count = 0;
            self.window_start = now;
        }
        self.count += 1;
        self.count <= self.max_per_min
    }
}

/// Constant-time token comparison to prevent timing-based token oracle attacks.
/// Returns `true` if `a == b` without short-circuiting on mismatch.
fn tokens_equal(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

// ─── JSON-RPC 2.0 types ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct RpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

// ─── Error codes — must match ClawdError in clawd_proto/lib/src/rpc.dart ────
//
// sessionNotFound      = -32001
// providerNotAvailable = -32002  (session busy — a turn is in progress)
// rateLimited          = -32003  (rate-limit from AI provider, not user-initiated)
// unauthorized         = -32004
// repoNotFound         = -32005
// sessionPaused        = -32006  (session is paused — call session.resume first)
// sessionLimitReached  = -32007  (max session count reached)

const PARSE_ERROR: i32 = -32700;
const INVALID_REQUEST: i32 = -32600;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;
const INTERNAL_ERROR: i32 = -32603;
const UNAUTHORIZED: i32 = -32004;
const SESSION_NOT_FOUND: i32 = -32001;
const REPO_NOT_FOUND: i32 = -32005;
/// Session is currently running — cannot accept a new message turn.
const SESSION_BUSY: i32 = -32002;
/// AI provider rate-limit response — client should retry after a delay.
const RATE_LIMITED: i32 = -32003;
/// Session is paused — must call session.resume before sending messages.
const SESSION_PAUSED_CODE: i32 = -32006;
/// Max session count reached — delete an existing session before creating a new one.
/// NOTE: clawd_proto ClawdError.sessionLimitReached must also use -32007.
const SESSION_LIMIT_CODE: i32 = -32007;
/// Tool call blocked by the security allowlist/denylist/denied-path policy (DC.T40).
const TOOL_SECURITY_CODE: i32 = -32028;
/// IPC-level connection or RPC rate limit exceeded (distinct from RATE_LIMITED = -32003
/// which is the AI provider rate limit).
const IPC_RATE_LIMITED_CODE: i32 = -32029;
// Task system error codes (-32010 through -32015)
const TASK_NOT_FOUND_CODE: i32 = -32010;
const TASK_ALREADY_CLAIMED_CODE: i32 = -32011;
#[allow(dead_code)]
const TASK_ALREADY_DONE_CODE: i32 = -32012;
#[allow(dead_code)]
const AGENT_NOT_FOUND_CODE: i32 = -32013;
const MISSING_COMPLETION_NOTES_CODE: i32 = -32014;
const TASK_NOT_RESUMABLE_CODE: i32 = -32015;
/// Tool rejected because the session is in FORGE or STORM mode (V02.T26).
/// NOTE: spec originally listed -32006 here, but -32006 = sessionPaused — using -32016.
#[allow(dead_code)]
pub const MODE_VIOLATION_CODE: i32 = -32016;

// ─── Server ──────────────────────────────────────────────────────────────────

pub async fn run(ctx: Arc<AppContext>) -> Result<()> {
    let addr = format!("{}:{}", ctx.config.bind_address, ctx.config.port);
    let listener = TcpListener::bind(&addr).await?;
    info!(addr = %addr, "IPC server listening (WebSocket + HTTP health on same port)");

    // Broadcast daemon.ready to anyone who subscribes after connect
    ctx.broadcaster.broadcast(
        "daemon.ready",
        serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "port": ctx.config.port
        }),
    );

    // Per-IP connection rate limiter (shared across all accept iterations).
    let conn_limiter = Arc::new(Mutex::new(ConnectionRateLimiter::new()));

    // Graceful shutdown: resolve on SIGTERM (Unix) or Ctrl-C (all platforms).
    // Pinned so we can use it in the select! loop without moving.
    let shutdown = make_shutdown_future();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            biased;

            _ = &mut shutdown => {
                info!("shutdown signal received — draining sessions and stopping IPC server");
                ctx.session_manager.drain().await;
                break;
            }

            conn = listener.accept() => {
                let (stream, peer) = match conn {
                    Ok(c) => c,
                    Err(e) => {
                        error!(err = %e, "accept error");
                        continue;
                    }
                };

                // Per-IP connection rate limit check (loopback is always exempt).
                {
                    let limit = ctx.config.security.max_connections_per_minute_per_ip as usize;
                    let mut limiter = conn_limiter.lock().await;
                    if !limiter.check_and_record(peer.ip(), limit) {
                        warn!(peer = %peer, "connection rate limit exceeded — rejecting");
                        ctx.metrics.inc_ipc_rate_limit_hits();
                        drop(stream);
                        continue;
                    }
                }

                debug!(peer = %peer, "new connection");
                let peer_ip = peer.ip();
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, peer_ip, ctx).await {
                        warn!(peer = %peer, err = %e, "connection error");
                    }
                });
            }
        }
    }

    info!("IPC server stopped");
    Ok(())
}

/// Respond to an HTTP `GET /health` request with a JSON status document.
///
/// The daemon shares port 4300 for both WebSocket (JSON-RPC) and a plain
/// HTTP health endpoint so clients can check liveness without a WS library.
async fn handle_health_check(mut stream: tokio::net::TcpStream, ctx: &AppContext) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // Consume the request headers (stack buffer — we don't inspect the body).
    let mut req_buf = [0u8; 256];
    let _ = stream.read(&mut req_buf).await;

    let uptime_secs = ctx.started_at.elapsed().as_secs();
    let active = ctx.session_manager.active_count().await;
    let body = serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": uptime_secs,
        "activeSessions": active,
        "port": ctx.config.port,
    });
    let body_str = body.to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body_str.len(),
        body_str
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

/// Respond to an HTTP `GET /metrics` request with Prometheus text format (DC.T49).
///
/// Accessible without auth — metrics are considered non-sensitive operational data.
/// No active session IDs or user content is included.
async fn handle_metrics(mut stream: tokio::net::TcpStream, ctx: &AppContext) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // Consume the request headers.
    let mut req_buf = [0u8; 256];
    let _ = stream.read(&mut req_buf).await;

    let active = ctx.session_manager.active_count().await as u64;
    let body = ctx.metrics.render_prometheus(active);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

/// Returns a future that resolves when a shutdown signal is received.
///
/// On Unix we listen for SIGTERM *and* Ctrl-C.
/// On other platforms we listen for Ctrl-C only.
async fn make_shutdown_future() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("failed to register SIGTERM");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.ok();
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    peer_ip: IpAddr,
    ctx: Arc<AppContext>,
) -> Result<()> {
    // Peek at the first bytes to distinguish HTTP health checks from WebSocket upgrades.
    // Both share the same port. An HTTP GET starts with "GET "; WS upgrade also starts
    // with "GET " but has an "Upgrade: websocket" header — we detect health checks by
    // looking for paths that don't have WebSocket headers.
    //
    // Simpler approach: peek for "GET /health " (with trailing space) specifically.
    // Checking 12 bytes prevents false matches on paths like "GET /health-check".
    // All other GET requests (including WebSocket upgrades) fall through to the WS handshake.
    let mut peek_buf = [0u8; 13];
    let n = stream.peek(&mut peek_buf).await.unwrap_or(0);
    if n >= 12 && &peek_buf[..12] == b"GET /health " {
        return handle_health_check(stream, &ctx).await;
    }
    if n >= 13 && &peek_buf[..13] == b"GET /metrics " {
        return handle_metrics(stream, &ctx).await;
    }

    let ws_config = WebSocketConfig {
        max_message_size: Some(16 * 1024 * 1024), // 16 MB
        max_frame_size: Some(4 * 1024 * 1024),    // 4 MB per frame
        ..Default::default()
    };
    let ws = accept_async_with_config(stream, Some(ws_config)).await?;
    let (mut sink, mut stream) = ws.split();

    // ── Auth challenge ───────────────────────────────────────────────────────
    // The first message from every client must be a `daemon.auth` RPC call
    // carrying the correct token.  This prevents other local processes from
    // connecting to the daemon and issuing arbitrary RPC commands.
    //
    // Token is stored at {data_dir}/auth_token with mode 0600.  The Flutter
    // desktop/mobile app reads this file and sends it here on every connect.
    //
    // We save the token the client authenticated with so we can re-verify it
    // on every subsequent RPC dispatch (supports auth token rotation).
    let mut client_token = String::new();
    if !ctx.auth_token.is_empty() {
        let first = tokio::time::timeout(std::time::Duration::from_secs(10), stream.next()).await;

        let text = match first {
            Ok(Some(Ok(Message::Text(t)))) => t,
            // Timeout, connection closed, or non-text frame — reject silently.
            _ => return Ok(()),
        };

        // Parse the RPC request
        let req: RpcRequest = match serde_json::from_str(&text) {
            Ok(r) => r,
            Err(_) => {
                let _ = sink
                    .send(Message::Text(error_response(
                        Value::Null,
                        PARSE_ERROR,
                        "Parse error",
                    )))
                    .await;
                return Ok(());
            }
        };

        let id = req.id.clone().unwrap_or(Value::Null);

        if req.method != "daemon.auth" {
            let _ = sink
                .send(Message::Text(error_response(
                    id,
                    UNAUTHORIZED,
                    "Unauthorized — send daemon.auth first",
                )))
                .await;
            return Ok(());
        }

        let provided = req
            .params
            .as_ref()
            .and_then(|p| p.get("token"))
            .and_then(Value::as_str)
            .unwrap_or_default();

        if !tokens_equal(provided, &ctx.auth_token) {
            let _ = sink
                .send(Message::Text(error_response(
                    id,
                    UNAUTHORIZED,
                    "Unauthorized — invalid token",
                )))
                .await;
            return Ok(());
        }

        // Auth success — save token for per-RPC re-validation, then respond.
        client_token = provided.to_string();
        let resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "authenticated": true }
        });
        let _ = sink.send(Message::Text(resp.to_string())).await;
        debug!("client authenticated");
    }

    let mut broadcast_rx = ctx.broadcaster.subscribe();
    let mut rpc_limiter = RpcRateLimiter::new(
        ctx.config.security.max_rpc_calls_per_minute_per_ip,
        peer_ip.is_loopback(),
    );

    loop {
        tokio::select! {
            // Incoming message from client
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Per-connection RPC rate limit.
                        if !rpc_limiter.check() {
                            ctx.metrics.inc_ipc_rate_limit_hits();
                            let resp = error_response(Value::Null, IPC_RATE_LIMITED_CODE, "RPC rate limit exceeded — try again shortly");
                            if let Err(e) = sink.send(Message::Text(resp)).await {
                                warn!(err = %e, "send error");
                                break;
                            }
                            continue;
                        }
                        let response = dispatch_text(&text, &ctx, &client_token).await;
                        if let Err(e) = sink.send(Message::Text(response)).await {
                            warn!(err = %e, "send error");
                            break;
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = sink.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        warn!(err = %e, "ws error");
                        break;
                    }
                    _ => {}
                }
            }
            // Outgoing broadcast event
            event = broadcast_rx.recv() => {
                match event {
                    Ok(json) => {
                        if let Err(e) = sink.send(Message::Text(json)).await {
                            warn!(err = %e, "broadcast send error");
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        // Slow client could not keep up with broadcast rate.
                        // Events are dropped for this client; the sender is
                        // never blocked. Log and continue rather than
                        // disconnecting, so the client can still send RPCs.
                        warn!(skipped = n, "broadcast lagged — slow client skipped events");
                    }
                }
            }
        }
    }
    Ok(())
}

/// Dispatch a raw JSON-RPC text frame.
///
/// `client_token` is the bearer token the client presented during `daemon.auth`.
/// On each call we re-verify it against `ctx.auth_token` so that token rotation
/// immediately invalidates in-flight connections.  Relay connections pass `""`
/// to skip this check (they authenticate at the relay layer instead).
pub(crate) async fn dispatch_text(text: &str, ctx: &AppContext, client_token: &str) -> String {
    // Parse
    let req: RpcRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(_) => {
            return error_response(Value::Null, PARSE_ERROR, "Parse error");
        }
    };

    // Re-validate bearer token on every RPC dispatch.
    // Accepts either the static daemon auth_token OR a valid (non-revoked) paired device token.
    // An empty client_token is no longer exempt: relay-proxied connections must
    // include the daemon bearer token in their JSON-RPC frames, just like local clients.
    if !ctx.auth_token.is_empty() {
        let is_daemon_token = tokens_equal(client_token, &ctx.auth_token);
        let is_device_token = if !is_daemon_token && !client_token.is_empty() {
            let pairing_storage =
                crate::pairing::storage::PairingStorage::new(ctx.storage.clone_pool());
            pairing_storage
                .get_by_token(client_token)
                .await
                .unwrap_or(None)
                .is_some()
        } else {
            false
        };
        if !is_daemon_token && !is_device_token {
            return error_response(
                req.id.unwrap_or(Value::Null),
                UNAUTHORIZED,
                "Unauthorized — invalid or missing token",
            );
        }
    }

    // Validate jsonrpc field
    if req.jsonrpc != "2.0" {
        return error_response(
            req.id.unwrap_or(Value::Null),
            INVALID_REQUEST,
            "Invalid Request",
        );
    }

    let id = req.id.unwrap_or(Value::Null);
    let params = req.params.unwrap_or(Value::Null);

    trace!(method = %req.method, "rpc dispatch");
    ctx.metrics.inc_rpc_requests();

    let result = dispatch(&req.method, params, ctx).await;

    match result {
        Ok(value) => {
            let resp = RpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(value),
                error: None,
            };
            serde_json::to_string(&resp).unwrap_or_default()
        }
        Err(e) => {
            // Map specific errors to RPC codes
            let (code, msg) = classify_error(&e, &req.method);
            error_response(id, code, &msg)
        }
    }
}

async fn dispatch(method: &str, params: Value, ctx: &AppContext) -> anyhow::Result<Value> {
    match method {
        // ─── Multi-account management ─────────────────────────────────────────
        "account.list" => handlers::accounts::list(params, ctx).await,
        "account.create" => handlers::accounts::create(params, ctx).await,
        "account.delete" => handlers::accounts::delete(params, ctx).await,
        "account.setPriority" => handlers::accounts::set_priority(params, ctx).await,
        "account.history" => handlers::accounts::history(params, ctx).await,
        // ─── License tier gating ─────────────────────────────────────────────
        "license.get" => handlers::license::get(params, ctx).await,
        "license.check" => handlers::license::check(params, ctx).await,
        "license.tier" => handlers::license::tier(params, ctx).await,
        "daemon.ping" => handlers::daemon::ping(params, ctx).await,
        "daemon.status" => handlers::daemon::status(params, ctx).await,
        "daemon.checkUpdate" => handlers::daemon::check_update(params, ctx).await,
        "daemon.applyUpdate" => handlers::daemon::apply_update(params, ctx).await,
        "daemon.updatePolicy" => handlers::daemon::update_policy(params, ctx).await,
        "daemon.setUpdatePolicy" => handlers::daemon::set_update_policy(params, ctx).await,
        "daemon.checkProvider" => handlers::provider::check_provider(params, ctx).await,
        "daemon.providers" => handlers::daemon::providers(params, ctx).await,
        // Sprint BB UX.3
        "daemon.changelog" => handlers::daemon::changelog(params, ctx).await,
        "repo.list" => handlers::repo::list(params, ctx).await,
        "repo.open" => handlers::repo::open(params, ctx).await,
        "repo.close" => handlers::repo::close(params, ctx).await,
        "repo.status" => handlers::repo::status(params, ctx).await,
        "repo.diff" => handlers::repo::diff(params, ctx).await,
        "repo.fileDiff" => handlers::repo::file_diff(params, ctx).await,
        "repo.tree" => handlers::repo::tree(params, ctx).await,
        "repo.readFile" => handlers::repo::read_file(params, ctx).await,
        // ─── Session Intelligence (Sprint G) ──────────────────────────────────
        "message.pin" => handlers::session_intelligence::pin_message(params, ctx).await,
        "message.unpin" => handlers::session_intelligence::unpin_message(params, ctx).await,
        "session.contextStatus" => {
            handlers::session_intelligence::context_status(params, ctx).await
        }
        "session.health" => handlers::session_intelligence::session_health(params, ctx).await,
        "session.splitProposed" => {
            handlers::session_intelligence::split_proposed(params, ctx).await
        }
        "context.bridge" => handlers::session_intelligence::context_bridge(params, ctx).await,
        // ─── Model Intelligence (Sprint H) ────────────────────────────────────
        "session.setModel" => handlers::model_intelligence::set_model(params, ctx).await,
        "session.addRepoContext" => {
            handlers::model_intelligence::add_repo_context(params, ctx).await
        }
        "session.listRepoContexts" => {
            handlers::model_intelligence::list_repo_contexts(params, ctx).await
        }
        "session.removeRepoContext" => {
            handlers::model_intelligence::remove_repo_context(params, ctx).await
        }
        // ─── Repo Intelligence (Sprint F) ─────────────────────────────────────
        "repo.scan" => handlers::repo_intelligence::scan(params, ctx).await,
        "repo.profile" => handlers::repo_intelligence::profile(params, ctx).await,
        "repo.generateArtifacts" => {
            handlers::repo_intelligence::generate_artifacts(params, ctx).await
        }
        "repo.syncArtifacts" => handlers::repo_intelligence::sync_artifacts(params, ctx).await,
        "repo.driftScore" => handlers::repo_intelligence::drift_score(params, ctx).await,
        "repo.driftReport" => handlers::repo_intelligence::drift_report(params, ctx).await,
        "validators.list" => handlers::repo_intelligence::validators_list(params, ctx).await,
        "validators.run" => handlers::repo_intelligence::validators_run(params, ctx).await,
        "session.create" => handlers::session::create(params, ctx).await,
        "session.list" => handlers::session::list(params, ctx).await,
        "session.get" => handlers::session::get(params, ctx).await,
        "session.delete" => handlers::session::delete(params, ctx).await,
        "session.sendMessage" => handlers::session::send_message(params, ctx).await,
        "session.getMessages" => handlers::session::get_messages(params, ctx).await,
        "session.pause" => handlers::session::pause(params, ctx).await,
        "session.resume" => handlers::session::resume(params, ctx).await,
        "session.cancel" => handlers::session::cancel(params, ctx).await,
        "session.setProvider" => handlers::session::set_provider(params, ctx).await,
        "session.setMode" => handlers::session::set_mode(params, ctx).await,
        // ─── Token usage ─────────────────────────────────────────────────────
        "token.sessionUsage" => handlers::token::session_usage(params, ctx).await,
        "token.totalUsage" => handlers::token::total_usage(params, ctx).await,
        "token.budgetStatus" => handlers::token::budget_status(params, ctx).await,
        "tool.approve" => handlers::tool::approve(params, ctx).await,
        "tool.reject" => handlers::tool::reject(params, ctx).await,
        // ─── Tool call audit log (DC.T43) ─────────────────────────────────────
        "session.toolCallAudit" => handlers::audit::tool_call_audit(params, ctx).await,
        // ─── Task system ─────────────────────────────────────────────────────
        "tasks.list" => handlers::tasks::list(params, ctx).await,
        "tasks.get" => handlers::tasks::get(params, ctx).await,
        "tasks.claim" => handlers::tasks::claim(params, ctx).await,
        "tasks.release" => handlers::tasks::release(params, ctx).await,
        "tasks.heartbeat" => handlers::tasks::heartbeat(params, ctx).await,
        "tasks.updateStatus" => handlers::tasks::update_status(params, ctx).await,
        "tasks.addTask" => handlers::tasks::add_task(params, ctx).await,
        "tasks.bulkAdd" => handlers::tasks::bulk_add(params, ctx).await,
        "tasks.logActivity" => handlers::tasks::log_activity(params, ctx).await,
        "tasks.note" => handlers::tasks::note(params, ctx).await,
        "tasks.activity" => handlers::tasks::activity(params, ctx).await,
        "tasks.fromPlanning" => handlers::tasks::from_planning(params, ctx).await,
        "tasks.fromChecklist" => handlers::tasks::from_checklist(params, ctx).await,
        "tasks.summary" => handlers::tasks::summary(params, ctx).await,
        "tasks.progressEstimate" => handlers::tasks::progress_estimate(params, ctx).await,
        "tasks.export" => handlers::tasks::export(params, ctx).await,
        "tasks.validate" => handlers::tasks::validate(params, ctx).await,
        "tasks.sync" => handlers::tasks::sync(params, ctx).await,
        // ─── Phase 43b: Task State Engine ────────────────────────────────────
        "tasks.createSpec" => handlers::tasks::create_from_spec(params, ctx).await,
        "tasks.transition" => handlers::tasks::transition(params, ctx).await,
        "tasks.listEvents" => handlers::tasks::list_events(params, ctx).await,
        // ─── Agent registry ──────────────────────────────────────────────────
        "tasks.agents.register" => handlers::agents::register(params, ctx).await,
        "tasks.agents.list" => handlers::agents::list(params, ctx).await,
        "tasks.agents.heartbeat" => handlers::agents::heartbeat(params, ctx).await,
        "tasks.agents.disconnect" => handlers::agents::disconnect(params, ctx).await,
        // ─── Phase 43e: Multi-agent orchestration ─────────────────────────────────
        "agents.spawn" => handlers::agents::spawn_agent(params, ctx).await,
        "agents.list" => handlers::agents::list_orchestrated(params, ctx).await,
        "agents.cancel" => handlers::agents::cancel_agent(params, ctx).await,
        "agents.heartbeat" => handlers::agents::orchestrator_heartbeat(params, ctx).await,
        // ─── AFS ─────────────────────────────────────────────────────────────
        "afs.init" => handlers::afs::init(params, ctx).await,
        "afs.status" => handlers::afs::status(params, ctx).await,
        "afs.syncInstructions" => handlers::afs::sync_instructions(params, ctx).await,
        "afs.register" => handlers::afs::register_project(params, ctx).await,
        // ─── Drift scanner (V02.T21-T23) ─────────────────────────────────────
        "drift.scan" => handlers::drift::scan(params, ctx).await,
        "drift.list" => handlers::drift::list(params, ctx).await,
        // ─── Coding standards (V02.T29-T31) ───────────────────────────────────
        "standards.list" => handlers::standards::list(params, ctx).await,
        // ─── Provider knowledge (V02.T33-T35) ─────────────────────────────────
        "providers.detect" => handlers::providers_handler::detect(params, ctx).await,
        "providers.list" => handlers::providers_handler::list(params, ctx).await,
        // ─── Doctor (D64) ─────────────────────────────────────────────────────
        "doctor.scan" => handlers::doctor::scan(params, ctx).await,
        "doctor.fix" => handlers::doctor::fix(params, ctx).await,
        "doctor.approveRelease" => handlers::doctor::approve_release(params, ctx).await,
        "doctor.hookInstall" => handlers::doctor::hook_install(params, ctx).await,
        // ─── Observability / Traces ───────────────────────────────────────────
        "traces.query" => handlers::telemetry::query_traces(params, ctx).await,
        "traces.summary" => handlers::telemetry::summary(params, ctx).await,
        // ─── Task Worktrees ───────────────────────────────────────────────────
        "worktrees.create" => handlers::worktrees::create(params, ctx).await,
        "worktrees.list" => handlers::worktrees::list(params, ctx).await,
        "worktrees.diff" => handlers::worktrees::diff(params, ctx).await,
        "worktrees.commit" => handlers::worktrees::commit(params, ctx).await,
        "worktrees.accept" => handlers::worktrees::accept(params, ctx).await,
        "worktrees.reject" => handlers::worktrees::reject(params, ctx).await,
        "worktrees.delete" => handlers::worktrees::delete(params, ctx).await,
        "worktrees.merge" => handlers::worktrees::merge(params, ctx).await,
        "worktrees.cleanup" => handlers::worktrees::cleanup(params, ctx).await,
        // ─── Human-approval workflow ──────────────────────────────────────────
        "approval.list" => handlers::approval::list(params, ctx).await,
        "approval.respond" => handlers::approval::respond(params, ctx).await,
        // ─── Phase 43m: Account Scheduler ────────────────────────────────────
        "scheduler.status" => handlers::scheduler::status(params, ctx).await,
        // ─── Phase 43f: Conversation Threading ───────────────────────────────────
        "threads.start" => handlers::threads::start_thread(ctx, params).await,
        "threads.resume" => handlers::threads::resume_thread(ctx, params).await,
        "threads.fork" => handlers::threads::fork_thread(ctx, params).await,
        "threads.list" => handlers::threads::list_threads(ctx, params).await,
        // ─── Phase 44: Resource Governor ─────────────────────────────────────
        "system.resources" => handlers::system::resources(params, ctx).await,
        "system.resourceHistory" => handlers::system::resource_history(params, ctx).await,
        // ─── Phase 45: Task Engine ─────────────────────────────────────────────────────
        "te.phase.create" => handlers::task_engine::phase_create(params, ctx).await,
        "te.phase.list" => handlers::task_engine::phase_list(params, ctx).await,
        "te.task.create" => handlers::task_engine::task_create(params, ctx).await,
        "te.task.get" => handlers::task_engine::task_get(params, ctx).await,
        "te.task.list" => handlers::task_engine::task_list(params, ctx).await,
        "te.task.transition" => handlers::task_engine::task_transition(params, ctx).await,
        "te.task.claim" => handlers::task_engine::task_claim(params, ctx).await,
        "te.agent.register" => handlers::task_engine::agent_register(params, ctx).await,
        "te.agent.heartbeat" => handlers::task_engine::agent_heartbeat(params, ctx).await,
        "te.agent.deregister" => handlers::task_engine::agent_deregister(params, ctx).await,
        "te.event.log" => handlers::task_engine::event_log(params, ctx).await,
        "te.event.list" => handlers::task_engine::event_list(params, ctx).await,
        "te.checkpoint.write" => handlers::task_engine::checkpoint_write(params, ctx).await,
        "te.note.add" => handlers::task_engine::note_add(params, ctx).await,
        "te.note.list" => handlers::task_engine::note_list(params, ctx).await,
        // ─── Phase 56: Projects ──────────────────────────────────────────────
        "project.create" => crate::project::handlers::project_create(params, ctx).await,
        "project.list" => crate::project::handlers::project_list(params, ctx).await,
        "project.get" => crate::project::handlers::project_get(params, ctx).await,
        "project.update" => crate::project::handlers::project_update(params, ctx).await,
        "project.delete" => crate::project::handlers::project_delete(params, ctx).await,
        "project.addRepo" => crate::project::handlers::project_add_repo(params, ctx).await,
        "project.removeRepo" => crate::project::handlers::project_remove_repo(params, ctx).await,
        "daemon.setName" => crate::project::handlers::daemon_set_name(params, ctx).await,
        // ─── Phase 56: Device Pairing ─────────────────────────────────────────
        "daemon.pairPin" => crate::pairing::handlers::pairing_generate_pin(params, ctx).await,
        "device.pair" => crate::pairing::handlers::device_pair(params, ctx).await,
        "device.list" => crate::pairing::handlers::device_list(params, ctx).await,
        "device.revoke" => crate::pairing::handlers::device_revoke(params, ctx).await,
        "device.rename" => crate::pairing::handlers::device_rename(params, ctx).await,

        // ─── Sprint I: Provider Onboarding ────────────────────────────────────
        "onboarding.checkAll" => {
            crate::providers_onboarding::handlers::check_all(params, ctx).await
        }
        "onboarding.checkProvider" => {
            crate::providers_onboarding::handlers::check_provider(params, ctx).await
        }
        "onboarding.addApiKey" => {
            crate::providers_onboarding::handlers::add_api_key(params, ctx).await
        }
        "onboarding.capabilities" => {
            crate::providers_onboarding::handlers::account_capabilities(params, ctx).await
        }
        "onboarding.generateGci" => {
            crate::providers_onboarding::handlers::generate_gci(params, ctx).await
        }
        "onboarding.generateCodexMd" => {
            crate::providers_onboarding::handlers::generate_codex_md(params, ctx).await
        }
        "onboarding.generateCursorRules" => {
            crate::providers_onboarding::handlers::generate_cursor_rules(params, ctx).await
        }
        "onboarding.bootstrapAid" => {
            crate::providers_onboarding::handlers::bootstrap_aid(params, ctx).await
        }
        "onboarding.checkAid" => {
            crate::providers_onboarding::handlers::check_aid(params, ctx).await
        }

        // ─── Sprint J: Autonomous Execution Engine ────────────────────────────
        "ae.plan.create" => crate::autonomous::handlers::ae_plan_create(params, ctx).await,
        "ae.plan.approve" => crate::autonomous::handlers::ae_plan_approve(params, ctx).await,
        "ae.plan.get" => crate::autonomous::handlers::ae_plan_get(params, ctx).await,
        "ae.decision.record" => crate::autonomous::handlers::ae_decision_record(params, ctx).await,
        "ae.confidence.get" => crate::autonomous::handlers::ae_confidence_get(params, ctx).await,
        "ae.recipe.list" => crate::autonomous::handlers::recipe_list(params, ctx).await,
        "ae.recipe.create" => crate::autonomous::handlers::recipe_create(params, ctx).await,

        // ─── Sprint K: Arena Mode + Code Completion ───────────────────────────
        "arena.create" => crate::arena::handlers::create_arena_session(params, ctx).await,
        "arena.vote" => crate::arena::handlers::record_vote(params, ctx).await,
        "arena.leaderboard" => crate::arena::handlers::get_leaderboard(params, ctx).await,
        "completion.suggest" => crate::completion::handlers::suggest_completion(params, ctx).await,

        // ─── Sprint M: Pack Marketplace ───────────────────────────────────────
        "packs.install" => handlers::packs::install(params, ctx).await,
        "packs.update" => handlers::packs::update(params, ctx).await,
        "packs.remove" => handlers::packs::remove(params, ctx).await,
        "packs.search" => handlers::packs::search(params, ctx).await,
        "packs.publish" => handlers::packs::publish(params, ctx).await,
        "packs.list" => handlers::packs::list_installed(params, ctx).await,

        // ─── Sprint N: Multi-Repo Orchestration ───────────────────────────────
        "mailbox.send" => crate::mailbox::handlers::mailbox_send(params, ctx).await,
        "mailbox.list" => crate::mailbox::handlers::mailbox_list(params, ctx).await,
        "mailbox.archive" => crate::mailbox::handlers::mailbox_archive(params, ctx).await,
        "topology.get" => crate::topology::handlers::topology_get(params, ctx).await,
        "topology.validate" => crate::topology::handlers::topology_validate(params, ctx).await,
        "topology.addDependency" => {
            crate::topology::handlers::topology_add_dependency(params, ctx).await
        }
        "topology.removeDependency" => {
            crate::topology::handlers::topology_remove_dependency(params, ctx).await
        }
        "topology.crossValidate" => {
            crate::topology::handlers::topology_cross_validate(params, ctx).await
        }

        // ─── Sprint O: AI Code Review Engine ─────────────────────────────────
        "review.run" => crate::code_review::handlers::run(params, ctx).await,
        "review.fix" => crate::code_review::handlers::fix(params, ctx).await,
        "review.learn" => crate::code_review::handlers::learn(params, ctx).await,

        // ─── Sprint P: Builder Mode ───────────────────────────────────────────
        "builder.create" => crate::builder::handlers::builder_create_session(params, ctx).await,
        "builder.templates" => crate::builder::handlers::builder_list_templates(params, ctx).await,
        "builder.status" => crate::builder::handlers::builder_get_status(params, ctx).await,

        // ─── Sprint Q: Analytics ──────────────────────────────────────────────
        "analytics.personal" => crate::analytics::handlers::personal(params, ctx).await,
        "analytics.providers" => crate::analytics::handlers::provider_breakdown(params, ctx).await,
        "analytics.session" => crate::analytics::handlers::session(params, ctx).await,
        "analytics.achievements" => {
            crate::analytics::handlers::achievements_list(params, ctx).await
        }
        // Sprint BB PV.18
        "analytics.budget" => crate::analytics::handlers::budget(params, ctx).await,

        // Sprint BC DL.3 — dead-letter queue
        "dead_letter.list" => handlers::dead_letter::list(params, ctx).await,
        "dead_letter.retry" => handlers::dead_letter::retry(params, ctx).await,

        // ─── Sprint CC: Quality Infrastructure ───────────────────────────────
        // CA — Task Automations
        "automation.list" => handlers::automations::list(params, ctx).await,
        "automation.trigger" => handlers::automations::trigger(params, ctx).await,
        "automation.disable" => handlers::automations::disable(params, ctx).await,
        // EV — Session Evals
        "eval.list" => handlers::evals::eval_list(params, ctx).await,
        "eval.run" => handlers::evals::eval_run(params, ctx).await,
        // TG — Task Genealogy
        "task.spawn" => handlers::tasks::spawn(params, ctx).await,
        "task.lineage" => handlers::tasks::lineage(params, ctx).await,
        // AM — Attention Map
        "session.attentionMap" => handlers::session::attention_map(params, ctx).await,
        // IE — Intent vs Execution
        "session.intentSummary" => handlers::session::intent_summary(params, ctx).await,
        // GD — Ghost Diff
        "ghost_diff.check" => handlers::ghost_diff::check(params, ctx).await,

        // ─── Sprint DD: Workflow Recipes ──────────────────────────────────────
        "workflow.create" => handlers::workflow::create(params, ctx).await,
        "workflow.list" => handlers::workflow::list(params, ctx).await,
        "workflow.run" => handlers::workflow::run(params, ctx).await,
        "workflow.delete" => handlers::workflow::delete(params, ctx).await,

        // ─── Sprint DD: Tool Sovereignty ──────────────────────────────────────
        "sovereignty.report" => handlers::sovereignty::report(params, ctx).await,
        "sovereignty.events" => handlers::sovereignty::events(params, ctx).await,

        // ─── Sprint DD: Project Pulse ─────────────────────────────────────────
        "project.pulse" => handlers::pulse::pulse(params, ctx).await,

        // ─── Sprint DD: Session Replay ────────────────────────────────────────
        "session.export" => handlers::replay::export(params, ctx).await,
        "session.import" => handlers::replay::import_bundle(params, ctx).await,
        "session.replay" => handlers::replay::replay(params, ctx).await,

        // ─── Sprint DD: Natural Language Git ─────────────────────────────────
        "git.query" => handlers::nl_git::query(params, ctx).await,

        // ─── Sprint EE: CI Runner ─────────────────────────────────────────────
        "ci.run" => handlers::ci::run(params, ctx).await,
        "ci.status" => handlers::ci::status(params, ctx).await,
        "ci.cancel" => handlers::ci::cancel(params, ctx).await,

        // ─── Sprint EE: Session Sharing ───────────────────────────────────────
        "session.share" => handlers::session_share::share(params, ctx).await,
        "session.revokeShare" => handlers::session_share::revoke_share(params, ctx).await,
        "session.shareList" => handlers::session_share::share_list(params, ctx).await,

        // ─── Sprint EE: Daily Digest ──────────────────────────────────────────
        "digest.today" => handlers::digest::today(params, ctx).await,

        // ─── Sprint S: LSP + VS Code compatibility ────────────────────────────
        "lsp.start" => crate::lsp::handlers::lsp_start(params, ctx).await,
        "lsp.stop" => crate::lsp::handlers::lsp_stop(params, ctx).await,
        "lsp.diagnostics" => crate::lsp::handlers::lsp_diagnostics(params, ctx).await,
        "lsp.completions" => crate::lsp::handlers::lsp_completions(params, ctx).await,
        "lsp.list" => crate::lsp::handlers::lsp_list_servers(params, ctx).await,

        // ─── Sprint L: Browser Tool (Visual & Multimodal) ────────────────────
        "browser.screenshot" => crate::browser_tool::handlers::screenshot(params, ctx).await,

        // ─── Sprint V: Prompt Intelligence ───────────────────────────────────
        "prompt.suggest" => crate::prompt_intelligence::handlers::prompt_suggest(params, ctx).await,
        "prompt.recordUsed" => {
            crate::prompt_intelligence::handlers::prompt_record_used(params, ctx).await
        }

        // ─── Sprint Z: IDE Extension Host ─────────────────────────────────────
        "ide.extensionConnected" => crate::ide::handlers::extension_connected(params, ctx).await,
        "ide.editorContext" => crate::ide::handlers::editor_context(params, ctx).await,
        "ide.syncSettings" => crate::ide::handlers::sync_settings(params, ctx).await,
        "ide.listConnections" => crate::ide::handlers::list_connections(params, ctx).await,
        "ide.latestContext" => crate::ide::handlers::latest_context(params, ctx).await,

        // ─── Sprint JJ: Direct + VPN Connectivity ────────────────────────────
        "connectivity.status" => handlers::connectivity::status(params, ctx).await,

        // ─── Sprint OO: AI Memory + Personalization ───────────────────────────
        "memory.list" => handlers::memory::list(params, ctx).await,
        "memory.add" => handlers::memory::add(params, ctx).await,
        "memory.remove" => handlers::memory::remove(params, ctx).await,
        "memory.update" => handlers::memory::update(params, ctx).await,

        // ─── Sprint PP: Observability + Metrics ─────────────────────────────
        "metrics.list" => handlers::metrics::list(params, ctx).await,
        "metrics.summary" => handlers::metrics::summary(params, ctx).await,
        "metrics.rollups" => handlers::metrics::rollups(params, ctx).await,

        // ─── Sprint RR: Push notifications ──────────────────────────────────
        "push.register" => handlers::push::register(params, ctx).await,
        "push.unregister" => handlers::push::unregister(params, ctx).await,

        // ─── Sprint TT: Pack ratings ─────────────────────────────────────────
        "pack.rate" => handlers::pack_ratings::rate(params, ctx).await,

        // ─── Sprint ZZ: Agent OS ──────────────────────────────────────────────
        // Instruction graph
        "instructions.compile" => handlers::instructions::compile(ctx, params).await,
        "instructions.explain" => handlers::instructions::explain(ctx, params).await,
        "instructions.budgetReport" => handlers::instructions::budget_report(ctx, params).await,
        "instructions.import" => handlers::instructions::import_project(ctx, params).await,
        "instructions.lint" => handlers::instructions::lint(ctx, params).await,
        "instructions.propose" => handlers::instructions::propose(ctx, params).await,
        "instructions.accept" => handlers::instructions::accept(ctx, params).await,
        "instructions.dismiss" => handlers::instructions::dismiss(ctx, params).await,
        "instructions.snapshot" => handlers::instructions::snapshot(ctx, params).await,
        "instructions.snapshotCheck" => handlers::instructions::snapshot_check(ctx, params).await,
        "instructions.doctor" => handlers::instructions::doctor(ctx, params).await,
        // Lease heartbeat + ownership (LH.T02, FO.T04)
        "task.heartbeat" => {
            let task_id = params["task_id"].as_str().unwrap_or("").to_string();
            let extend_secs = params["extend_secs"].as_i64().unwrap_or(300);
            let new_expires =
                crate::tasks::janitor::extend_lease(&ctx.storage, &task_id, extend_secs).await?;
            Ok(serde_json::json!({ "task_id": task_id, "lease_expires_at": new_expires }))
        }
        "task.expandOwnership" => {
            let task_id = params["task_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing task_id"))?
                .to_string();
            let new_patterns: Vec<String> = params["patterns"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let ownership = crate::tasks::ownership::OwnershipStorage::new(&ctx.storage);
            let expanded = ownership.expand_ownership(&task_id, &new_patterns).await?;
            Ok(serde_json::json!({ "task_id": task_id, "owned_paths_json": expanded }))
        }
        // Evidence packs
        "task.evidencePack" => handlers::artifacts::evidence_pack(ctx, params).await,
        // Security / injection defense
        "security.analyzeContent" => handlers::security::analyze_content_rpc(ctx, params).await,
        "security.testInjection" => handlers::security::test_injection(ctx, params).await,
        // Diff risk
        "review.diffRisk" => handlers::review_risk::diff_risk(ctx, params).await,
        // Policy tests
        "policy.test" => handlers::policy::test(ctx, params).await,
        "policy.seedTests" => handlers::policy::seed_tests(ctx, params).await,
        // Benchmark harness
        "bench.run" => handlers::bench::run(ctx, params).await,
        "bench.compare" => handlers::bench::compare(ctx, params).await,
        "bench.list" => handlers::bench::list(ctx, params).await,
        "bench.seedTasks" => handlers::bench::seed_tasks(ctx, params).await,
        // OpenTelemetry traces
        "session.trace" => {
            let session_id = params["session_id"].as_str().unwrap_or("").to_string();
            let spans =
                crate::session::telemetry::get_session_trace(&ctx.storage, &session_id).await?;
            Ok(
                serde_json::json!({ "spans": spans.iter().map(|s| serde_json::json!({
                "span_id": s.span_id,
                "parent_span_id": s.parent_span_id,
                "trace_id": s.trace_id,
                "name": s.name,
                "attributes": s.attributes,
                "started_at_ms": s.started_at_ms,
                "duration_ms": s.duration_ms,
                "status": format!("{:?}", s.status),
            })).collect::<Vec<_>>() }),
            )
        }
        // EP.T04 — Evidence pack retrieval
        "artifacts.evidencePack" => handlers::artifacts::evidence_pack(ctx, params).await,

        // Provider capability matrix
        "providers.listCapabilities" => {
            use crate::agents::capabilities::{Provider, ProviderCapabilities};
            let provider_pairs = [("claude", Provider::Claude), ("codex", Provider::Codex)];
            let list = provider_pairs
                .iter()
                .map(|(name, p)| {
                    let caps = ProviderCapabilities::for_provider(p);
                    serde_json::json!({
                        "name": name,
                        "supports_fork": caps.supports_fork,
                        "supports_resume": caps.supports_resume,
                        "supports_mcp": caps.supports_mcp,
                        "supports_sandbox": caps.supports_sandbox,
                        "supports_worktree": caps.supports_worktree,
                        "max_context_tokens": caps.max_context_tokens,
                        "cost_per_1k_in": caps.cost_per_1k_tokens_in,
                        "cost_per_1k_out": caps.cost_per_1k_tokens_out,
                    })
                })
                .collect::<Vec<_>>();
            Ok(serde_json::json!({ "providers": list }))
        }

        _ => Err(anyhow::anyhow!("METHOD_NOT_FOUND:{}", method)),
    }
}

fn classify_error(e: &anyhow::Error, _method: &str) -> (i32, String) {
    let msg = e.to_string();

    // ── Structured prefixes (reliable, added at the error callsite) ──────────

    if msg.starts_with("METHOD_NOT_FOUND:") {
        return (METHOD_NOT_FOUND, "Method not found".to_string());
    }

    // Task system uses "TASK_CODE:{numeric_code}" markers.
    if msg.contains(&format!(
        "TASK_CODE:{}",
        crate::tasks::storage::TASK_NOT_FOUND
    )) {
        return (TASK_NOT_FOUND_CODE, "Task not found".to_string());
    }
    if msg.contains(&format!(
        "TASK_CODE:{}",
        crate::tasks::storage::TASK_ALREADY_CLAIMED
    )) {
        return (
            TASK_ALREADY_CLAIMED_CODE,
            "Task already claimed by another agent".to_string(),
        );
    }
    if msg.contains(&format!(
        "TASK_CODE:{}",
        crate::tasks::storage::MISSING_COMPLETION_NOTES
    )) {
        return (
            MISSING_COMPLETION_NOTES_CODE,
            "Completion notes are required when marking a task done".to_string(),
        );
    }
    if msg.contains(&format!(
        "TASK_CODE:{}",
        crate::tasks::storage::TASK_NOT_RESUMABLE
    )) {
        return (
            TASK_NOT_RESUMABLE_CODE,
            "Task cannot be resumed — not in interrupted or pending state".to_string(),
        );
    }

    // ── All-caps sentinel markers (set explicitly by each error site) ─────────

    if msg.contains("SESSION_NOT_FOUND") {
        return (SESSION_NOT_FOUND, "Session not found".to_string());
    }
    if msg.contains("SESSION_LIMIT_REACHED") {
        return (SESSION_LIMIT_CODE, "Session limit reached".to_string());
    }
    if msg.contains("REPO_NOT_FOUND") {
        return (REPO_NOT_FOUND, "Repo not found".to_string());
    }
    if msg.contains("PROVIDER_NOT_AVAILABLE") {
        let detail = msg
            .split_once("PROVIDER_NOT_AVAILABLE: ")
            .map(|x| x.1)
            .unwrap_or("Provider not available");
        return (SESSION_BUSY, detail.to_string());
    }
    if msg.contains("SESSION_BUSY") {
        return (
            SESSION_BUSY,
            "Session is busy — cancel or wait for the current turn".to_string(),
        );
    }
    if msg.contains("SESSION_PAUSED") {
        return (
            SESSION_PAUSED_CODE,
            "Session is paused — resume before sending messages".to_string(),
        );
    }
    // Tool call blocked by security policy (DC.T40): allowlist, denylist, or denied path.
    if msg.contains("TOOL_DENIED:")
        || msg.contains("TOOL_NOT_ALLOWED:")
        || msg.contains("TOOL_PATH_DENIED:")
    {
        return (
            TOOL_SECURITY_CODE,
            "Tool call blocked by security policy".to_string(),
        );
    }

    if msg.contains("MODE_VIOLATION") {
        let mode = msg
            .split_once("MODE_VIOLATION: ")
            .map(|x| x.1)
            .unwrap_or("FORGE or STORM");
        return (
            MODE_VIOLATION_CODE,
            format!("Tool rejected — session is in {mode} mode (write operations blocked)"),
        );
    }
    if msg.contains("RATE_LIMITED") {
        return (
            RATE_LIMITED,
            "AI provider rate limit — try again shortly".to_string(),
        );
    }

    // ── Fallback heuristics for legacy error strings not yet converted ────────
    // These are less reliable; prefer adding a structured marker above for new errors.

    if msg.contains("session limit reached") {
        return (SESSION_LIMIT_CODE, "Session limit reached".to_string());
    }
    if msg.contains("session not found") {
        return (SESSION_NOT_FOUND, "Session not found".to_string());
    }
    if msg.contains("repo not found") || msg.contains("not a git repository") {
        return (REPO_NOT_FOUND, "Repo not found".to_string());
    }
    if msg.contains("rate limit") || msg.contains("rate_limit") {
        return (
            RATE_LIMITED,
            "AI provider rate limit — try again shortly".to_string(),
        );
    }
    if msg.contains("missing field") || msg.contains("invalid type") {
        return (INVALID_PARAMS, format!("Invalid params: {}", msg));
    }

    // ── Catch-all ─────────────────────────────────────────────────────────────
    error!(err = %e, "internal error");
    (INTERNAL_ERROR, "Internal error".to_string())
}

/// Strip the user's home directory from file paths in error messages
/// to avoid leaking the full filesystem layout in RPC responses.
fn sanitize_path_in_message(msg: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return msg.replace(&home, "~");
        }
    }
    msg.to_string()
}

fn error_response(id: Value, code: i32, message: &str) -> String {
    let sanitized = sanitize_path_in_message(message);
    let resp = RpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(RpcError {
            code,
            message: sanitized,
        }),
    };
    serde_json::to_string(&resp).unwrap_or_default()
}
