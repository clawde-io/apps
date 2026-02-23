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
use tokio_tungstenite::{accept_async_with_config, tungstenite::{protocol::WebSocketConfig, Message}};
use tracing::{debug, error, info, trace, warn};

// ─── Rate limiting ──────────────────────────────────────────────────────────

/// Max new WebSocket connections per IP per minute.
const MAX_CONNECTIONS_PER_MIN: usize = 10;
/// Max RPC requests per connection per second.
const MAX_RPC_PER_SEC: u32 = 100;

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
    fn check_and_record(&mut self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let one_min_ago = now - std::time::Duration::from_secs(60);

        let timestamps = self.connections.entry(ip).or_default();
        timestamps.retain(|t| *t > one_min_ago);

        if timestamps.len() >= MAX_CONNECTIONS_PER_MIN {
            return false;
        }
        timestamps.push(now);
        true
    }
}

/// Per-connection RPC rate tracker using a tumbling window (resets each second).
struct RpcRateLimiter {
    count: u32,
    window_start: Instant,
}

impl RpcRateLimiter {
    fn new() -> Self {
        Self {
            count: 0,
            window_start: Instant::now(),
        }
    }

    /// Returns `true` if the request should be allowed.
    fn check(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.window_start).as_secs() >= 1 {
            self.count = 0;
            self.window_start = now;
        }
        self.count += 1;
        self.count <= MAX_RPC_PER_SEC
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
// Task system error codes (-32010 through -32015)
const TASK_NOT_FOUND_CODE: i32 = -32010;
const TASK_ALREADY_CLAIMED_CODE: i32 = -32011;
#[allow(dead_code)]
const TASK_ALREADY_DONE_CODE: i32 = -32012;
#[allow(dead_code)]
const AGENT_NOT_FOUND_CODE: i32 = -32013;
const MISSING_COMPLETION_NOTES_CODE: i32 = -32014;
const TASK_NOT_RESUMABLE_CODE: i32 = -32015;

// ─── Server ──────────────────────────────────────────────────────────────────

pub async fn run(ctx: Arc<AppContext>) -> Result<()> {
    let addr = format!("127.0.0.1:{}", ctx.config.port);
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

                // Per-IP connection rate limit check.
                {
                    let mut limiter = conn_limiter.lock().await;
                    if !limiter.check_and_record(peer.ip()) {
                        warn!(peer = %peer, "connection rate limit exceeded — rejecting");
                        drop(stream);
                        continue;
                    }
                }

                debug!(peer = %peer, "new connection");
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, ctx).await {
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

async fn handle_connection(stream: tokio::net::TcpStream, ctx: Arc<AppContext>) -> Result<()> {
    // Peek at the first bytes to distinguish HTTP health checks from WebSocket upgrades.
    // Both share the same port. An HTTP GET starts with "GET "; WS upgrade also starts
    // with "GET " but has an "Upgrade: websocket" header — we detect health checks by
    // looking for paths that don't have WebSocket headers.
    //
    // Simpler approach: peek for "GET /health " (with trailing space) specifically.
    // Checking 12 bytes prevents false matches on paths like "GET /health-check".
    // All other GET requests (including WebSocket upgrades) fall through to the WS handshake.
    let mut peek_buf = [0u8; 12];
    let n = stream.peek(&mut peek_buf).await.unwrap_or(0);
    if n >= 12 && &peek_buf[..12] == b"GET /health " {
        return handle_health_check(stream, &ctx).await;
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
    let mut rpc_limiter = RpcRateLimiter::new();

    loop {
        tokio::select! {
            // Incoming message from client
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Per-connection RPC rate limit.
                        if !rpc_limiter.check() {
                            let resp = error_response(Value::Null, RATE_LIMITED, "RPC rate limit exceeded — max 100 req/sec");
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
    // If daemon auth is configured, every connection must present the correct token.
    // An empty client_token is no longer exempt: relay-proxied connections must
    // include the daemon bearer token in their JSON-RPC frames, just like local clients.
    if !ctx.auth_token.is_empty() && !tokens_equal(client_token, &ctx.auth_token) {
        return error_response(
            req.id.unwrap_or(Value::Null),
            UNAUTHORIZED,
            "Unauthorized — invalid or missing token",
        );
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
        "daemon.ping" => handlers::daemon::ping(params, ctx).await,
        "daemon.status" => handlers::daemon::status(params, ctx).await,
        "daemon.checkUpdate" => handlers::daemon::check_update(params, ctx).await,
        "daemon.applyUpdate" => handlers::daemon::apply_update(params, ctx).await,
        "repo.open" => handlers::repo::open(params, ctx).await,
        "repo.close" => handlers::repo::close(params, ctx).await,
        "repo.status" => handlers::repo::status(params, ctx).await,
        "repo.diff" => handlers::repo::diff(params, ctx).await,
        "repo.fileDiff" => handlers::repo::file_diff(params, ctx).await,
        "session.create" => handlers::session::create(params, ctx).await,
        "session.list" => handlers::session::list(params, ctx).await,
        "session.get" => handlers::session::get(params, ctx).await,
        "session.delete" => handlers::session::delete(params, ctx).await,
        "session.sendMessage" => handlers::session::send_message(params, ctx).await,
        "session.getMessages" => handlers::session::get_messages(params, ctx).await,
        "session.pause" => handlers::session::pause(params, ctx).await,
        "session.resume" => handlers::session::resume(params, ctx).await,
        "session.cancel" => handlers::session::cancel(params, ctx).await,
        "tool.approve" => handlers::tool::approve(params, ctx).await,
        "tool.reject" => handlers::tool::reject(params, ctx).await,
        // ─── Task system ─────────────────────────────────────────────────────
        "tasks.list"           => handlers::tasks::list(params, ctx).await,
        "tasks.get"            => handlers::tasks::get(params, ctx).await,
        "tasks.claim"          => handlers::tasks::claim(params, ctx).await,
        "tasks.release"        => handlers::tasks::release(params, ctx).await,
        "tasks.heartbeat"      => handlers::tasks::heartbeat(params, ctx).await,
        "tasks.updateStatus"   => handlers::tasks::update_status(params, ctx).await,
        "tasks.addTask"        => handlers::tasks::add_task(params, ctx).await,
        "tasks.bulkAdd"        => handlers::tasks::bulk_add(params, ctx).await,
        "tasks.logActivity"    => handlers::tasks::log_activity(params, ctx).await,
        "tasks.note"           => handlers::tasks::note(params, ctx).await,
        "tasks.activity"       => handlers::tasks::activity(params, ctx).await,
        "tasks.fromPlanning"   => handlers::tasks::from_planning(params, ctx).await,
        "tasks.fromChecklist"  => handlers::tasks::from_checklist(params, ctx).await,
        "tasks.summary"        => handlers::tasks::summary(params, ctx).await,
        "tasks.export"         => handlers::tasks::export(params, ctx).await,
        "tasks.validate"       => handlers::tasks::validate(params, ctx).await,
        "tasks.sync"           => handlers::tasks::sync(params, ctx).await,
        // ─── Phase 43b: Task State Engine ────────────────────────────────────
        "tasks.createSpec"     => handlers::tasks::create_from_spec(params, ctx).await,
        "tasks.transition"     => handlers::tasks::transition(params, ctx).await,
        "tasks.listEvents"     => handlers::tasks::list_events(params, ctx).await,
        // ─── Agent registry ──────────────────────────────────────────────────
        "tasks.agents.register"    => handlers::agents::register(params, ctx).await,
        "tasks.agents.list"        => handlers::agents::list(params, ctx).await,
        "tasks.agents.heartbeat"   => handlers::agents::heartbeat(params, ctx).await,
        "tasks.agents.disconnect"  => handlers::agents::disconnect(params, ctx).await,
        // ─── Phase 43e: Multi-agent orchestration ─────────────────────────────────
        "agents.spawn"      => handlers::agents::spawn_agent(params, ctx).await,
        "agents.list"       => handlers::agents::list_orchestrated(params, ctx).await,
        "agents.cancel"     => handlers::agents::cancel_agent(params, ctx).await,
        "agents.heartbeat"  => handlers::agents::orchestrator_heartbeat(params, ctx).await,
        // ─── AFS ─────────────────────────────────────────────────────────────
        "afs.init"              => handlers::afs::init(params, ctx).await,
        "afs.status"            => handlers::afs::status(params, ctx).await,
        "afs.syncInstructions"  => handlers::afs::sync_instructions(params, ctx).await,
        "afs.register"          => handlers::afs::register_project(params, ctx).await,
        // ─── Observability / Traces ───────────────────────────────────────────
        "traces.query"          => handlers::telemetry::query_traces(params, ctx).await,
        "traces.summary"        => handlers::telemetry::summary(params, ctx).await,
        // ─── Phase 43c: Task Worktrees ────────────────────────────────────────
        "worktrees.list"        => handlers::worktrees::list(params, ctx).await,
        "worktrees.merge"       => handlers::worktrees::merge(params, ctx).await,
        "worktrees.cleanup"     => handlers::worktrees::cleanup(params, ctx).await,
        "worktrees.diff"        => handlers::worktrees::diff(params, ctx).await,
        // ─── Human-approval workflow ──────────────────────────────────────────
        "approval.list"         => handlers::approval::list(params, ctx).await,
        "approval.respond"      => handlers::approval::respond(params, ctx).await,
        // ─── Phase 43m: Account Scheduler ────────────────────────────────────
        "scheduler.status"      => handlers::scheduler::status(params, ctx).await,
        // ─── Phase 43f: Conversation Threading ───────────────────────────────────
        "threads.start"         => handlers::threads::start_thread(ctx, params).await,
        "threads.resume"        => handlers::threads::resume_thread(ctx, params).await,
        "threads.fork"          => handlers::threads::fork_thread(ctx, params).await,
        "threads.list"          => handlers::threads::list_threads(ctx, params).await,
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
    if msg.contains(&format!("TASK_CODE:{}", crate::tasks::storage::TASK_NOT_FOUND)) {
        return (TASK_NOT_FOUND_CODE, "Task not found".to_string());
    }
    if msg.contains(&format!("TASK_CODE:{}", crate::tasks::storage::TASK_ALREADY_CLAIMED)) {
        return (TASK_ALREADY_CLAIMED_CODE, "Task already claimed by another agent".to_string());
    }
    if msg.contains(&format!("TASK_CODE:{}", crate::tasks::storage::MISSING_COMPLETION_NOTES)) {
        return (MISSING_COMPLETION_NOTES_CODE, "Completion notes are required when marking a task done".to_string());
    }
    if msg.contains(&format!("TASK_CODE:{}", crate::tasks::storage::TASK_NOT_RESUMABLE)) {
        return (TASK_NOT_RESUMABLE_CODE, "Task cannot be resumed — not in interrupted or pending state".to_string());
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
    if msg.contains("RATE_LIMITED") {
        return (RATE_LIMITED, "AI provider rate limit — try again shortly".to_string());
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
        return (RATE_LIMITED, "AI provider rate limit — try again shortly".to_string());
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
