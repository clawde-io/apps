/// MCP client — connects to upstream MCP servers via stdio subprocess.
///
/// `McpClient` spawns a child process (the MCP server binary) and communicates
/// with it over its stdin/stdout pipes using the JSON-RPC 2.0 MCP protocol.
/// Each request gets a monotonically incrementing integer ID.
///
/// Trust levels: `Trusted` servers are invoked as-is.  `Untrusted` server
/// responses are scanned for prompt-injection patterns before being returned
/// to callers.
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use super::tools::McpToolDef;

// ─── Trust level ─────────────────────────────────────────────────────────────

/// How much to trust an upstream MCP server's responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpTrustLevel {
    /// The server is controlled by the user or us.  Responses are passed
    /// through verbatim.
    Trusted,
    /// Third-party server.  Responses are scanned for prompt-injection
    /// patterns and redacted before being returned.
    Untrusted,
}

// ─── Server config ────────────────────────────────────────────────────────────

/// Configuration for one upstream MCP server.
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// Display name for logging and UI.
    pub name: String,
    /// Executable to run (e.g. `"npx"`, `"python"`, `"/usr/local/bin/my-mcp"`).
    pub command: String,
    /// Arguments passed to the command.
    pub args: Vec<String>,
    /// Environment variables to inject into the child process.
    pub env: HashMap<String, String>,
    /// Trust level — controls whether prompt-injection scanning is applied.
    pub trust: McpTrustLevel,
}

// ─── Prompt-injection patterns ────────────────────────────────────────────────

/// Simple string patterns that indicate a possible prompt-injection attempt
/// in an untrusted server response.  If any are found the entire `data` field
/// is replaced with a redaction notice.
const INJECTION_PATTERNS: &[&str] = &[
    "IGNORE PREVIOUS INSTRUCTIONS",
    "IGNORE ALL PREVIOUS",
    "DISREGARD PREVIOUS",
    "OVERRIDE INSTRUCTIONS",
    "YOU ARE NOW",
    "NEW INSTRUCTIONS",
    "SYSTEM PROMPT:",
    "<<SYS>>",
    "[INST]",
    "</s>",
];

fn contains_injection(text: &str) -> bool {
    let upper = text.to_uppercase();
    INJECTION_PATTERNS.iter().any(|p| upper.contains(p))
}

fn sanitize_value(v: Value) -> Value {
    match &v {
        Value::String(s) if contains_injection(s) => {
            Value::String("[REDACTED: potential prompt injection detected]".into())
        }
        Value::Object(map) => {
            let sanitized: serde_json::Map<String, Value> = map
                .iter()
                .map(|(k, val)| (k.clone(), sanitize_value(val.clone())))
                .collect();
            Value::Object(sanitized)
        }
        Value::Array(arr) => {
            Value::Array(arr.iter().map(|val| sanitize_value(val.clone())).collect())
        }
        _ => v,
    }
}

// ─── McpClient ────────────────────────────────────────────────────────────────

/// A live connection to one upstream MCP server process.
pub struct McpClient {
    config: McpServerConfig,
    /// The child process (kept alive for the duration of the connection).
    _child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    next_id: AtomicU64,
}

impl McpClient {
    /// Spawn a new MCP server subprocess and open its stdio pipes.
    ///
    /// Sends the MCP `initialize` handshake immediately after spawning.
    pub async fn spawn(config: McpServerConfig) -> Result<Self> {
        let mut cmd = tokio::process::Command::new(&config.command);
        cmd.args(&config.args);
        for (k, v) in &config.env {
            cmd.env(k, v);
        }
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::null());

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn MCP server '{}'", config.name))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("MCP server stdin not available"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("MCP server stdout not available"))?;

        let client = Self {
            config,
            _child: child,
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: Arc::new(Mutex::new(BufReader::new(stdout))),
            next_id: AtomicU64::new(1),
        };

        // MCP initialize handshake.
        client.initialize().await?;

        Ok(client)
    }

    // ─── Internals ──────────────────────────────────────────────────────────

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a JSON-RPC request and read back the response line.
    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id();
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params.unwrap_or(Value::Null)
        });
        let mut line = serde_json::to_string(&req)?;
        line.push('\n');

        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(line.as_bytes())
                .await
                .context("write to MCP server stdin")?;
            stdin.flush().await?;
        }

        // Read response — MCP servers send one JSON object per line.
        let response_line = {
            let mut stdout = self.stdout.lock().await;
            let mut buf = String::new();
            stdout
                .read_line(&mut buf)
                .await
                .context("read from MCP server stdout")?;
            buf
        };

        if response_line.is_empty() {
            return Err(anyhow::anyhow!(
                "MCP server '{}' closed stdout unexpectedly",
                self.config.name
            ));
        }

        let resp: Value = serde_json::from_str(response_line.trim())
            .context("parse MCP server response")?;

        if let Some(error) = resp.get("error") {
            return Err(anyhow::anyhow!(
                "MCP server returned error: {}",
                error
            ));
        }

        Ok(resp.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Send the MCP `initialize` request.
    async fn initialize(&self) -> Result<()> {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "clawd",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        let result = self.send_request("initialize", Some(params)).await?;
        debug!(
            server = %self.config.name,
            protocol = result.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or("?"),
            "MCP server initialized"
        );

        // Send the `initialized` notification (no response expected).
        let notif = json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        });
        let mut line = serde_json::to_string(&notif)?;
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;

        Ok(())
    }

    // ─── Public API ─────────────────────────────────────────────────────────

    /// List all tools available from this MCP server.
    pub async fn list_tools(&self) -> Result<Vec<McpToolDef>> {
        let result = self.send_request("tools/list", None).await?;
        let raw_tools = result
            .get("tools")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let tools: Vec<McpToolDef> = raw_tools
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();

        Ok(tools)
    }

    /// Call a tool on the upstream MCP server.
    ///
    /// If the server is `Untrusted`, the result is sanitized against
    /// prompt-injection patterns before being returned.
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        let params = json!({
            "name": name,
            "arguments": args
        });

        let result = self.send_request("tools/call", Some(params)).await?;

        // Sanitize untrusted responses.
        let final_result = if self.config.trust == McpTrustLevel::Untrusted {
            let sanitized = sanitize_value(result.clone());
            if sanitized != result {
                warn!(
                    server = %self.config.name,
                    tool = name,
                    "prompt injection pattern detected in untrusted MCP response — redacted"
                );
            }
            sanitized
        } else {
            result
        };

        Ok(final_result)
    }

    /// The display name of this server (from config).
    pub fn name(&self) -> &str {
        &self.config.name
    }
}
