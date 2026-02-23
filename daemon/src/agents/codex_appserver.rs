//! Codex app-server protocol: thread/start, thread/resume, thread/fork.
//!
//! `codex app-server` is a long-running JSON-RPC-over-stdio process that
//! supports named thread operations.  This module manages its lifecycle and
//! exposes a typed API for the three core operations.

use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;

// ─── Types ────────────────────────────────────────────────────────────────────

/// A reference to a Codex app-server thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexThread {
    pub thread_id: String,
    pub session_id: Option<String>,
}

// ─── CodexAppServer ───────────────────────────────────────────────────────────

/// A live connection to a `codex app-server` subprocess.
pub struct CodexAppServer {
    _child: Child,
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    request_id: AtomicU64,
}

impl CodexAppServer {
    /// Spawn `codex app-server` and open its stdio pipes.
    pub async fn spawn() -> Result<Self> {
        let mut child = tokio::process::Command::new("codex")
            .arg("app-server")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("failed to spawn codex app-server")?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("codex app-server stdin not available"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("codex app-server stdout not available"))?;

        Ok(Self {
            _child: child,
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            request_id: AtomicU64::new(1),
        })
    }

    // ─── Internal JSON-RPC framing ────────────────────────────────────────

    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a JSON-RPC 2.0 request and read back the response line.
    async fn send_request(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id();
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        let mut line = serde_json::to_string(&req)?;
        line.push('\n');

        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(line.as_bytes())
                .await
                .context("write to codex app-server stdin")?;
            stdin.flush().await.context("flush codex app-server stdin")?;
        }

        let response_line = {
            let mut stdout = self.stdout.lock().await;
            let mut buf = String::new();
            stdout
                .read_line(&mut buf)
                .await
                .context("read from codex app-server stdout")?;
            buf
        };

        if response_line.is_empty() {
            return Err(anyhow::anyhow!("codex app-server closed stdout unexpectedly"));
        }

        let resp: Value = serde_json::from_str(response_line.trim())
            .context("parse codex app-server response")?;

        if let Some(error) = resp.get("error") {
            return Err(anyhow::anyhow!("codex app-server error: {}", error));
        }

        Ok(resp.get("result").cloned().unwrap_or(Value::Null))
    }

    // ─── Public thread API ────────────────────────────────────────────────

    /// Start a new thread with the given instructions and model.
    ///
    /// Returns a `CodexThread` containing the allocated `thread_id`.
    pub async fn thread_start(&self, instructions: &str, model: &str) -> Result<CodexThread> {
        let params = json!({
            "instructions": instructions,
            "model": model
        });
        let result = self.send_request("thread/start", params).await?;
        let thread: CodexThread = serde_json::from_value(result)
            .context("parse thread/start response")?;
        Ok(thread)
    }

    /// Resume an existing thread by its ID.
    pub async fn thread_resume(&self, thread_id: &str) -> Result<CodexThread> {
        let params = json!({ "thread_id": thread_id });
        let result = self.send_request("thread/resume", params).await?;
        let thread: CodexThread = serde_json::from_value(result)
            .context("parse thread/resume response")?;
        Ok(thread)
    }

    /// Fork a thread to create a parallel branch for exploration.
    pub async fn thread_fork(&self, thread_id: &str) -> Result<CodexThread> {
        let params = json!({ "thread_id": thread_id });
        let result = self.send_request("thread/fork", params).await?;
        let thread: CodexThread = serde_json::from_value(result)
            .context("parse thread/fork response")?;
        Ok(thread)
    }
}
