//! Lightweight JSON-RPC WebSocket client for CLI commands.
//!
//! CLI subcommands (`clawd status`, `clawd account`, etc.) use this to connect
//! to the running daemon and call RPC methods with authentication.

use anyhow::{Context as _, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// A short-lived WebSocket client for CLI-to-daemon RPC calls.
///
/// Connects once, authenticates, then allows multiple `call()` invocations.
/// Drop to close the connection.
pub struct DaemonClient {
    url: String,
    port: u16,
    token: String,
}

impl DaemonClient {
    /// Create a client targeting the daemon on the given port with the given auth token.
    pub fn new(port: u16, token: String) -> Self {
        let url = format!("ws://127.0.0.1:{port}");
        Self { url, port, token }
    }

    /// Check if the daemon is reachable (3-second timeout).
    pub async fn is_reachable(&self) -> bool {
        let connect = connect_async(&self.url);
        matches!(
            tokio::time::timeout(std::time::Duration::from_secs(3), connect).await,
            Ok(Ok(_))
        )
    }

    /// Connect, authenticate, call one RPC method, and return the result.
    ///
    /// Uses a 5-second timeout for both connection and the RPC call.
    pub async fn call_once(&self, method: &str, params: Value) -> Result<Value> {
        let timeout = std::time::Duration::from_secs(5);
        let (mut ws, _) = tokio::time::timeout(timeout, connect_async(&self.url))
            .await
            .context("timed out connecting to daemon")?
            .context("failed to connect to daemon WebSocket")?;

        // Authenticate
        let auth_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "daemon.auth",
            "params": { "token": self.token }
        });
        ws.send(Message::Text(serde_json::to_string(&auth_req).unwrap()))
            .await?;

        // Wait for auth response
        let _auth_resp = self.read_response(&mut ws, 1).await?;

        // Make the actual call
        let req = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": method,
            "params": params
        });
        ws.send(Message::Text(serde_json::to_string(&req).unwrap()))
            .await?;

        self.read_response(&mut ws, 2).await
    }

    /// Read messages until we get the response with the given `id`.
    async fn read_response(
        &self,
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        id: u64,
    ) -> Result<Value> {
        let timeout = std::time::Duration::from_secs(5);
        loop {
            let msg = tokio::time::timeout(timeout, ws.next())
                .await
                .context("timed out waiting for daemon response")?
                .context("WebSocket stream ended")?
                .context("WebSocket error")?;

            if let Message::Text(text) = msg {
                let v: Value = serde_json::from_str(&text)?;
                if v.get("id").and_then(|x| x.as_u64()) == Some(id) {
                    if let Some(err) = v.get("error") {
                        anyhow::bail!("daemon RPC error: {err}");
                    }
                    return Ok(v["result"].clone());
                }
                // else: notification — skip and read next
            }
        }
    }

    /// Port the client is targeting.
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// Read the auth token from the daemon's data directory.
///
/// Returns an error if the file does not exist (daemon not yet initialized).
pub fn read_auth_token(data_dir: &std::path::Path) -> Result<String> {
    let token_path = data_dir.join("auth_token");
    let token = std::fs::read_to_string(&token_path).with_context(|| {
        format!(
            "could not read auth token from {path}\n  Is the daemon installed? Run `clawd service install` first.",
            path = token_path.display()
        )
    })?;
    Ok(token.trim().to_owned())
}
