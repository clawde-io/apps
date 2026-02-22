pub mod auth;
pub mod event;
pub mod handlers;

use crate::AppContext;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

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
/// Session is paused — must call session.resume before sending messages.
const SESSION_PAUSED_CODE: i32 = -32006;

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

    // Consume the request (we don't inspect it — any GET /health is fine).
    let mut req_buf = vec![0u8; 2048];
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
    // Simpler approach: peek for "GET /health" specifically. All other GET requests
    // (including WebSocket upgrades) fall through to the WS handshake as normal.
    let mut peek_buf = [0u8; 12];
    let n = stream.peek(&mut peek_buf).await.unwrap_or(0);
    if n >= 11 && &peek_buf[..11] == b"GET /health" {
        return handle_health_check(stream, &ctx).await;
    }

    let ws = accept_async(stream).await?;
    let (mut sink, mut stream) = ws.split();

    // ── Auth challenge ───────────────────────────────────────────────────────
    // The first message from every client must be a `daemon.auth` RPC call
    // carrying the correct token.  This prevents other local processes from
    // connecting to the daemon and issuing arbitrary RPC commands.
    //
    // Token is stored at {data_dir}/auth_token with mode 0600.  The Flutter
    // desktop/mobile app reads this file and sends it here on every connect.
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

        if provided != ctx.auth_token {
            let _ = sink
                .send(Message::Text(error_response(
                    id,
                    UNAUTHORIZED,
                    "Unauthorized — invalid token",
                )))
                .await;
            return Ok(());
        }

        // Auth success — send the RPC response and continue.
        let resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "authenticated": true }
        });
        let _ = sink.send(Message::Text(resp.to_string())).await;
        debug!("client authenticated");
    }

    let mut broadcast_rx = ctx.broadcaster.subscribe();

    loop {
        tokio::select! {
            // Incoming message from client
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let response = dispatch_text(&text, &ctx).await;
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
                        warn!(skipped = n, "broadcast lagged");
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) async fn dispatch_text(text: &str, ctx: &AppContext) -> String {
    // Parse
    let req: RpcRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(_) => {
            return error_response(Value::Null, PARSE_ERROR, "Parse error");
        }
    };

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

    debug!(method = %req.method, "rpc dispatch");

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
        _ => Err(anyhow::anyhow!("METHOD_NOT_FOUND:{}", method)),
    }
}

fn classify_error(e: &anyhow::Error, _method: &str) -> (i32, String) {
    let msg = e.to_string();
    if msg.starts_with("METHOD_NOT_FOUND:") {
        return (METHOD_NOT_FOUND, "Method not found".to_string());
    }
    if msg.contains("session limit reached") {
        return (INTERNAL_ERROR, format!("Session limit reached: {msg}"));
    }
    if msg.contains("session not found") || msg.contains("SESSION_NOT_FOUND") {
        return (SESSION_NOT_FOUND, "Session not found".to_string());
    }
    if msg.contains("repo not found")
        || msg.contains("REPO_NOT_FOUND")
        || msg.contains("not a git repository")
    {
        return (REPO_NOT_FOUND, "Repo not found".to_string());
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
    if msg.contains("missing field") || msg.contains("invalid type") {
        return (INVALID_PARAMS, format!("Invalid params: {}", msg));
    }
    error!(err = %e, "internal error");
    (INTERNAL_ERROR, "Internal error".to_string())
}

fn error_response(id: Value, code: i32, message: &str) -> String {
    let resp = RpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(RpcError {
            code,
            message: message.to_string(),
        }),
    };
    serde_json::to_string(&resp).unwrap_or_default()
}
