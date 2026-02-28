// SPDX-License-Identifier: MIT
// Sprint II CH.1 + CH.3 + CH.4 — `clawd chat` terminal REPL.
//
// Interactive AI chat directly in the terminal. Connects to the running daemon
// via JSON-RPC WebSocket, creates or resumes a session, and streams responses.
//
// Usage:
//   clawd chat                          # new interactive session
//   clawd chat --resume <session-id>    # resume an existing session
//   clawd chat --session-list           # pick from recent sessions
//   clawd chat --non-interactive "..."  # single-shot query, print response, exit

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::{json, Value};
use std::io::{self, Write as IoWrite};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::cli::chat_ui::ChatUi;
use crate::config::DaemonConfig;

/// Options for the `clawd chat` command.
#[derive(Debug, Default)]
pub struct ChatOpts {
    /// Resume an existing session by ID.
    pub resume: Option<String>,
    /// Print recent sessions and let the user pick one (interactive).
    pub session_list: bool,
    /// Single-shot non-interactive query — print response and exit.
    pub non_interactive: Option<String>,
    /// Provider to use when creating a new session (default: claude).
    pub provider: Option<String>,
}

/// Entry point for `clawd chat`.
pub async fn run_chat(opts: ChatOpts, config: &DaemonConfig) -> Result<()> {
    // Non-interactive mode: send one prompt, print response, exit.
    if let Some(prompt) = opts.non_interactive {
        return run_non_interactive(&prompt, &opts.provider, config).await;
    }

    // Interactive TUI mode.
    let session_id = if opts.session_list {
        pick_session(config).await?
    } else if let Some(id) = opts.resume {
        id
    } else {
        // Create a new session.
        create_session(&opts.provider, config).await?
    };

    ChatUi::new(session_id, config).run().await
}

// ─── Non-interactive mode ─────────────────────────────────────────────────────

/// Send a single prompt, print the response, and exit.
async fn run_non_interactive(
    prompt: &str,
    provider: &Option<String>,
    config: &DaemonConfig,
) -> Result<()> {
    let token = crate::cli::client::read_auth_token(&config.data_dir)?;
    let url = format!("ws://127.0.0.1:{}", config.port);

    let (mut ws, _) = tokio::time::timeout(std::time::Duration::from_secs(5), connect_async(&url))
        .await
        .context("timed out connecting to daemon")?
        .context("failed to connect to daemon")?;

    // Authenticate.
    ws_send(
        &mut ws,
        &json!({"jsonrpc":"2.0","id":1,"method":"daemon.auth","params":{"token":token}}),
    )
    .await?;
    ws_recv_id(&mut ws, 1).await?;

    // Create a session.
    let prov = provider.as_deref().unwrap_or("claude");
    ws_send(
        &mut ws,
        &json!({"jsonrpc":"2.0","id":2,"method":"session.create","params":{"provider":prov,"title":"clawd chat"}}),
    )
    .await?;
    let create_resp = ws_recv_id(&mut ws, 2).await?;
    let session_id = create_resp["id"]
        .as_str()
        .context("session.create: missing id")?
        .to_owned();

    // Show spinner while waiting.
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message("Thinking…");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    // Send the message.
    ws_send(
        &mut ws,
        &json!({"jsonrpc":"2.0","id":3,"method":"session.send","params":{"sessionId":session_id,"content":prompt}}),
    )
    .await?;
    ws_recv_id(&mut ws, 3).await?;

    // Collect streaming deltas until message.complete.
    let mut response = String::new();
    loop {
        let msg = tokio::time::timeout(std::time::Duration::from_secs(120), ws.next())
            .await
            .context("timed out waiting for response")?
            .context("stream ended")?
            .context("ws error")?;

        if let Message::Text(text) = msg {
            let v: Value = serde_json::from_str(&text)?;
            match v.get("method").and_then(|m| m.as_str()) {
                Some("session.message.delta") => {
                    let delta = v["params"]["delta"].as_str().unwrap_or("");
                    response.push_str(delta);
                }
                Some("session.message.complete") => break,
                _ => {}
            }
        }
    }

    spinner.finish_and_clear();
    println!("{response}");
    Ok(())
}

// ─── Session picker ───────────────────────────────────────────────────────────

/// List recent sessions and prompt the user to pick one.
async fn pick_session(config: &DaemonConfig) -> Result<String> {
    let token = crate::cli::client::read_auth_token(&config.data_dir)?;
    let client = crate::cli::client::DaemonClient::new(config.port, token);

    let result = client
        .call_once("session.list", json!({"limit": 10}))
        .await
        .context("session.list RPC failed")?;

    let sessions = result["sessions"]
        .as_array()
        .context("session.list: missing sessions array")?;

    if sessions.is_empty() {
        println!("No sessions found. Starting a new session.");
        return create_session(&None, config).await;
    }

    println!("Recent sessions:");
    for (i, s) in sessions.iter().enumerate() {
        let id = s["id"].as_str().unwrap_or("?");
        let title = s["title"].as_str().unwrap_or("(untitled)");
        let status = s["status"].as_str().unwrap_or("idle");
        println!("  [{i}] {title}  ({status})  {id}");
    }
    print!(
        "Pick a session [0-{}], or Enter for new: ",
        sessions.len() - 1
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return create_session(&None, config).await;
    }

    let idx: usize = trimmed.parse().context("invalid selection")?;
    let session = sessions.get(idx).context("selection out of range")?;
    Ok(session["id"].as_str().context("missing id")?.to_owned())
}

// ─── Session creation ─────────────────────────────────────────────────────────

async fn create_session(provider: &Option<String>, config: &DaemonConfig) -> Result<String> {
    let token = crate::cli::client::read_auth_token(&config.data_dir)?;
    let client = crate::cli::client::DaemonClient::new(config.port, token);
    let prov = provider.as_deref().unwrap_or("claude");

    let result = client
        .call_once(
            "session.create",
            json!({"provider": prov, "title": "clawd chat"}),
        )
        .await
        .context("session.create RPC failed")?;

    result["id"]
        .as_str()
        .context("session.create: missing id")
        .map(ToOwned::to_owned)
}

// ─── WebSocket helpers ────────────────────────────────────────────────────────

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn ws_send(ws: &mut WsStream, value: &Value) -> Result<()> {
    ws.send(Message::Text(serde_json::to_string(value)?))
        .await
        .context("ws send failed")
}

async fn ws_recv_id(ws: &mut WsStream, id: u64) -> Result<Value> {
    let timeout = std::time::Duration::from_secs(10);
    loop {
        let msg = tokio::time::timeout(timeout, ws.next())
            .await
            .context("timed out waiting for response")?
            .context("ws stream ended")?
            .context("ws error")?;

        if let Message::Text(text) = msg {
            let v: Value = serde_json::from_str(&text)?;
            if v.get("id").and_then(|x| x.as_u64()) == Some(id) {
                if let Some(err) = v.get("error") {
                    anyhow::bail!("RPC error: {err}");
                }
                return Ok(v["result"].clone());
            }
        }
    }
}
