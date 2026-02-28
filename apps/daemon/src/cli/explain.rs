// SPDX-License-Identifier: MIT
// Sprint II EX.1 + EX.2 + EX.3 — `clawd explain` terminal CLI.
//
// Creates an ephemeral AI session, asks the AI to explain a file, code range,
// stdin input, or error message, streams the explanation to the terminal, and
// exits. No session is persisted after the command completes.
//
// Usage:
//   clawd explain src/main.rs                 # explain the whole file
//   clawd explain src/main.rs --line 42       # explain around line 42
//   clawd explain src/main.rs --lines 40-60   # explain lines 40–60
//   clawd explain --stdin                     # read from stdin
//   clawd explain --error "E0308 ..."         # explain an error message
//   clawd explain src/lib.rs --format json    # structured JSON output

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read as IoRead;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::config::DaemonConfig;

/// Output format for `clawd explain`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExplainFormat {
    /// Plain text (default).
    #[default]
    Text,
    /// Structured JSON with explanation + suggestions.
    Json,
}

/// Options for `clawd explain`.
#[derive(Debug, Default)]
pub struct ExplainOpts {
    /// File path to explain (mutually exclusive with `stdin` and `error`).
    pub file: Option<std::path::PathBuf>,
    /// Specific line to focus on (1-based).
    pub line: Option<u32>,
    /// Line range to focus on — "start-end" (1-based, inclusive).
    pub lines: Option<String>,
    /// Read code from stdin instead of a file.
    pub stdin: bool,
    /// Explain an error message string.
    pub error: Option<String>,
    /// Output format.
    pub format: ExplainFormat,
    /// Provider to use (default: claude).
    pub provider: Option<String>,
}

/// Structured output for `--format json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExplainResponse {
    pub explanation: String,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Suggestion {
    pub action: String,
    pub code: Option<String>,
}

/// Entry point for `clawd explain`.
pub async fn run_explain(opts: ExplainOpts, config: &DaemonConfig) -> Result<()> {
    let prompt = build_prompt(&opts)?;
    let response = query_ai(&prompt, opts.provider.as_deref(), config).await?;

    match opts.format {
        ExplainFormat::Text => println!("{response}"),
        ExplainFormat::Json => {
            // Attempt to extract structured content; fall back to wrapping in JSON.
            let structured = ExplainResponse {
                explanation: response,
                suggestions: vec![],
            };
            println!("{}", serde_json::to_string_pretty(&structured)?);
        }
    }

    Ok(())
}

// ─── Prompt construction ──────────────────────────────────────────────────────

fn build_prompt(opts: &ExplainOpts) -> Result<String> {
    if let Some(ref error_msg) = opts.error {
        return Ok(format!(
            "Explain the following compiler/runtime error and suggest how to fix it:\n\n```\n{error_msg}\n```"
        ));
    }

    let code = if opts.stdin {
        read_stdin()?
    } else if let Some(ref path) = opts.file {
        if !path.exists() {
            anyhow::bail!("file not found: {}", path.display());
        }
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;
        extract_range(&content, opts.line, &opts.lines)
    } else {
        anyhow::bail!("specify a file, --stdin, or --error");
    };

    let file_label = opts
        .file
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<stdin>".to_owned());

    let range_label = if let Some(ref r) = opts.lines {
        format!(" (lines {r})")
    } else if let Some(n) = opts.line {
        format!(" (around line {n})")
    } else {
        String::new()
    };

    Ok(format!(
        "Explain the following code from `{file_label}`{range_label}. \
         Be concise and focus on what it does, any pitfalls, and how it could be improved:\n\n\
         ```\n{code}\n```"
    ))
}

/// Extract a line range from `content`.
fn extract_range(content: &str, line: Option<u32>, lines: &Option<String>) -> String {
    let all: Vec<&str> = content.lines().collect();

    let (start, end) = if let Some(ref range) = lines {
        if let Some((s, e)) = range.split_once('-') {
            let s: usize = s.trim().parse().unwrap_or(1);
            let e: usize = e.trim().parse().unwrap_or(all.len());
            (s.saturating_sub(1), e.min(all.len()))
        } else {
            (0, all.len())
        }
    } else if let Some(n) = line {
        let n = n as usize;
        let ctx = 10_usize;
        let s = n.saturating_sub(ctx + 1);
        let e = (n + ctx).min(all.len());
        (s, e)
    } else {
        (0, all.len())
    };

    all[start..end].join("\n")
}

fn read_stdin() -> Result<String> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("reading stdin")?;
    Ok(buf)
}

// ─── AI query ─────────────────────────────────────────────────────────────────

async fn query_ai(prompt: &str, provider: Option<&str>, config: &DaemonConfig) -> Result<String> {
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

    // Create ephemeral session.
    let prov = provider.unwrap_or("claude");
    ws_send(
        &mut ws,
        &json!({
            "jsonrpc":"2.0","id":2,
            "method":"session.create",
            "params":{"provider": prov, "title": "clawd explain", "ephemeral": true}
        }),
    )
    .await?;
    let create_resp = ws_recv_id(&mut ws, 2).await?;
    let session_id = create_resp["id"]
        .as_str()
        .context("session.create: missing id")?
        .to_owned();

    // Spinner.
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message("Explaining…");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    // Send prompt.
    ws_send(
        &mut ws,
        &json!({
            "jsonrpc":"2.0","id":3,
            "method":"session.send",
            "params":{"sessionId": session_id, "content": prompt}
        }),
    )
    .await?;
    ws_recv_id(&mut ws, 3).await?;

    // Collect streaming deltas.
    let mut response = String::new();
    loop {
        let msg = tokio::time::timeout(std::time::Duration::from_secs(120), ws.next())
            .await
            .context("timed out waiting for explanation")?
            .context("stream ended")?
            .context("ws error")?;

        if let Message::Text(text) = msg {
            let v: Value = serde_json::from_str(&text)?;
            match v.get("method").and_then(|m| m.as_str()) {
                Some("session.message.delta") => {
                    response.push_str(v["params"]["delta"].as_str().unwrap_or(""));
                }
                Some("session.message.complete") => break,
                _ => {}
            }
        }
    }

    spinner.finish_and_clear();
    Ok(response)
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
            .context("timed out")?
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

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_range_all() {
        let content = "line1\nline2\nline3\nline4\nline5";
        let result = extract_range(content, None, &None);
        assert_eq!(result, content);
    }

    #[test]
    fn extract_range_specific_lines() {
        let content = "a\nb\nc\nd\ne";
        let result = extract_range(content, None, &Some("2-4".to_owned()));
        assert_eq!(result, "b\nc\nd");
    }

    #[test]
    fn extract_range_single_line_context() {
        let content = (1..=30)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let result = extract_range(&content, Some(15), &None);
        // Should include lines around 15 (±10 lines).
        assert!(result.contains("line 15"));
    }

    #[test]
    fn build_prompt_error() {
        let opts = ExplainOpts {
            error: Some("E0308: mismatched types".to_owned()),
            ..Default::default()
        };
        let prompt = build_prompt(&opts).unwrap();
        assert!(prompt.contains("E0308"));
        assert!(prompt.to_lowercase().contains("explain"));
    }

    #[test]
    fn missing_source_returns_error() {
        let opts = ExplainOpts::default();
        assert!(build_prompt(&opts).is_err());
    }
}
