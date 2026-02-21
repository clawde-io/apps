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

// ─── Error codes (matching @clawde/proto) ───────────────────────────────────

const PARSE_ERROR: i32 = -32700;
const INVALID_REQUEST: i32 = -32600;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;
const INTERNAL_ERROR: i32 = -32603;
const SESSION_NOT_FOUND: i32 = -32000;
const REPO_NOT_FOUND: i32 = -32001;

// ─── Server ──────────────────────────────────────────────────────────────────

pub async fn run(ctx: Arc<AppContext>) -> Result<()> {
    let addr = format!("127.0.0.1:{}", ctx.config.port);
    let listener = TcpListener::bind(&addr).await?;
    info!(addr = %addr, "IPC server listening");

    // Broadcast daemon.ready to anyone who subscribes after connect
    ctx.broadcaster.broadcast(
        "daemon.ready",
        serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "port": ctx.config.port
        }),
    );

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(conn) => conn,
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

async fn handle_connection(
    stream: tokio::net::TcpStream,
    ctx: Arc<AppContext>,
) -> Result<()> {
    let ws = accept_async(stream).await?;
    let (mut sink, mut stream) = ws.split();
    let mut broadcast_rx = ctx.broadcaster.subscribe();

    loop {
        tokio::select! {
            // Incoming message from client
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let response = dispatch_text(&text, &ctx).await;
                        if let Err(e) = sink.send(Message::Text(response.into())).await {
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
                        if let Err(e) = sink.send(Message::Text(json.into())).await {
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

async fn dispatch_text(text: &str, ctx: &AppContext) -> String {
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
    if msg.contains("session not found") || msg.contains("SESSION_NOT_FOUND") {
        return (SESSION_NOT_FOUND, "Session not found".to_string());
    }
    if msg.contains("repo not found") || msg.contains("REPO_NOT_FOUND") {
        return (REPO_NOT_FOUND, "Repo not found".to_string());
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
