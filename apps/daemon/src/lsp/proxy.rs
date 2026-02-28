// SPDX-License-Identifier: MIT
/// LSP proxy — manages language server subprocess lifecycle and communication.
///
/// Sprint S, LS.T01–LS.T02.
///
/// Each `LspProxy` instance manages a pool of language server processes, one
/// per (language, workspace_root) pair.  Communication uses JSON-RPC 2.0 over
/// the subprocess's stdin/stdout (the stdio transport mandated by LSP 3.17).
///
/// # Design notes
///
/// - The proxy is intentionally simple: it does synchronous request/response
///   over stdio.  There is no async LSP push-notification loop — diagnostics
///   are fetched on demand via `textDocument/diagnostic` or by opening a file
///   and waiting for `textDocument/publishDiagnostics` notifications.
/// - Each `start_server` call spawns a child process and performs the LSP
///   `initialize` / `initialized` handshake.
/// - Requests are serialized with a monotonically increasing id so that
///   responses can be matched even when the server sends notifications
///   interleaved with responses.
use crate::lsp::model::{
    CompletionItem, DiagSeverity, DiagnosticItem, LspConfig, LspMessage, LspProcess,
};
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};

// ─── Internal server state ────────────────────────────────────────────────────

struct ServerState {
    process: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    language: String,
    workspace_root: String,
    next_id: AtomicU64,
}

impl ServerState {
    /// Send a JSON-RPC message via LSP stdio transport.
    ///
    /// The LSP stdio transport framing is:
    /// ```text
    /// Content-Length: <n>\r\n
    /// \r\n
    /// <n bytes of JSON>
    /// ```
    fn send(&mut self, msg: &LspMessage) -> Result<()> {
        let body = serde_json::to_string(msg)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        self.stdin.write_all(header.as_bytes())?;
        self.stdin.write_all(body.as_bytes())?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Read the next complete LSP message from the server's stdout.
    ///
    /// Parses the `Content-Length` header, then reads exactly that many bytes
    /// as the JSON body.
    fn recv(&mut self) -> Result<LspMessage> {
        // Read headers until blank line
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line)?;
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length: ") {
                content_length = Some(value.trim().parse()?);
            }
        }
        let length = content_length.context("LSP response missing Content-Length header")?;

        // Read exactly `length` bytes of JSON body
        let mut body = vec![0u8; length];
        use std::io::Read;
        self.stdout.read_exact(&mut body)?;

        let msg: LspMessage =
            serde_json::from_slice(&body).context("failed to parse LSP JSON body")?;
        Ok(msg)
    }

    /// Send a request and wait for the matching response (skipping notifications).
    fn request(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = LspMessage::request(id, method, params);
        self.send(&req)?;

        // Consume messages until we get a response matching our id
        loop {
            let msg = self.recv()?;
            // Skip notifications (no id field)
            if msg.id.is_none() {
                debug!(method = ?msg.method, "lsp notification (skipped)");
                continue;
            }
            // Match by id
            let resp_id = msg.id.as_ref().and_then(|v| v.as_u64()).unwrap_or(u64::MAX);
            if resp_id != id {
                debug!(resp_id, expected = id, "lsp response id mismatch, skipping");
                continue;
            }
            if let Some(error) = msg.error {
                bail!("LSP error response: {error}");
            }
            return msg.result.context("LSP response has no result");
        }
    }

    /// Allocate the next request id.
    #[allow(dead_code)]
    fn alloc_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Check whether the child process has exited.
    fn is_alive(&mut self) -> bool {
        self.process.try_wait().ok().flatten().is_none()
    }
}

// ─── Public proxy ─────────────────────────────────────────────────────────────

/// Manages a pool of LSP server processes across multiple languages and
/// workspace roots.
///
/// Safe to clone — the internal map is behind a `Mutex`.
#[derive(Clone)]
pub struct LspProxy {
    /// Map key: `"<language>:<workspace_root>"`.
    servers: Arc<Mutex<HashMap<String, ServerState>>>,
    /// Available LSP configs (built-ins + user overrides).
    configs: Vec<LspConfig>,
}

impl Default for LspProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl LspProxy {
    /// Create a new proxy with the default built-in language server configs.
    pub fn new() -> Self {
        Self {
            servers: Arc::new(Mutex::new(HashMap::new())),
            configs: LspConfig::builtin_defaults(),
        }
    }

    /// Create a proxy with a custom set of configs (built-ins are not added).
    pub fn with_configs(configs: Vec<LspConfig>) -> Self {
        Self {
            servers: Arc::new(Mutex::new(HashMap::new())),
            configs,
        }
    }

    fn server_key(language: &str, workspace_root: &Path) -> String {
        format!("{}:{}", language, workspace_root.display())
    }

    /// Find the LSP config for the given language name.
    fn find_config(&self, language: &str) -> Option<&LspConfig> {
        self.configs.iter().find(|c| c.language == language)
    }

    /// Spawn an LSP server for `language` at `workspace_root` and perform the
    /// `initialize` / `initialized` handshake.
    ///
    /// If a server for this (language, workspace) pair is already running and
    /// healthy, the existing process is returned without spawning a new one.
    pub fn start_server(&self, language: &str, workspace_root: &Path) -> Result<LspProcess> {
        let key = Self::server_key(language, workspace_root);

        let mut servers = self
            .servers
            .lock()
            .map_err(|_| anyhow::anyhow!("LSP proxy mutex poisoned"))?;

        // Return existing healthy server
        if let Some(existing) = servers.get_mut(&key) {
            if existing.is_alive() {
                return Ok(LspProcess {
                    language: existing.language.clone(),
                    pid: existing.process.id(),
                    workspace_root: existing.workspace_root.clone(),
                });
            }
            // Dead server — remove and restart
            warn!(language, "LSP server process exited, restarting");
            servers.remove(&key);
        }

        let config = self
            .find_config(language)
            .ok_or_else(|| anyhow::anyhow!("no LSP config for language: {language}"))?;

        let executable = config.server_command.first().ok_or_else(|| {
            anyhow::anyhow!("LSP server_command is empty for language: {language}")
        })?;

        info!(language, cmd = executable, "launching LSP server");

        let mut cmd = std::process::Command::new(executable);
        for arg in config.server_args.iter() {
            cmd.arg(arg);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .current_dir(workspace_root);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn LSP server for {language}: {executable}"))?;

        let stdin = child.stdin.take().context("child stdin not available")?;
        let stdout = child.stdout.take().context("child stdout not available")?;
        let pid = child.id();

        let mut state = ServerState {
            process: child,
            stdin,
            stdout: BufReader::new(stdout),
            language: language.to_string(),
            workspace_root: workspace_root.to_string_lossy().to_string(),
            next_id: AtomicU64::new(1),
        };

        // ── LSP initialize handshake ──────────────────────────────────────────

        let workspace_uri = format!("file://{}", workspace_root.display());
        let init_params = serde_json::json!({
            "processId": std::process::id(),
            "clientInfo": { "name": "clawd", "version": env!("CARGO_PKG_VERSION") },
            "rootUri": workspace_uri,
            "capabilities": {
                "textDocument": {
                    "synchronization": { "dynamicRegistration": false },
                    "completion": {
                        "completionItem": {
                            "snippetSupport": false,
                            "documentationFormat": ["plaintext"]
                        }
                    },
                    "hover": { "contentFormat": ["plaintext"] },
                    "publishDiagnostics": { "relatedInformation": false }
                },
                "workspace": {
                    "applyEdit": false,
                    "workspaceEdit": { "documentChanges": false }
                }
            },
            "workspaceFolders": [{ "uri": workspace_uri, "name": workspace_root.file_name()
                .unwrap_or_default()
                .to_string_lossy() }]
        });

        state.request("initialize", init_params)?;

        // Send the required `initialized` notification (no response expected)
        let initialized_notif = LspMessage::notification("initialized", serde_json::json!({}));
        state.send(&initialized_notif)?;

        debug!(language, pid, "LSP server initialized");

        let process_info = LspProcess {
            language: language.to_string(),
            pid,
            workspace_root: workspace_root.to_string_lossy().to_string(),
        };

        servers.insert(key, state);
        Ok(process_info)
    }

    /// Stop the LSP server for the given language and workspace root.
    ///
    /// Sends the `shutdown` request and `exit` notification per LSP spec,
    /// then removes the process from the pool.
    pub fn stop_server(&self, language: &str, workspace_root: &Path) -> Result<()> {
        let key = Self::server_key(language, workspace_root);
        let mut servers = self
            .servers
            .lock()
            .map_err(|_| anyhow::anyhow!("LSP proxy mutex poisoned"))?;

        if let Some(mut state) = servers.remove(&key) {
            // Best-effort shutdown handshake — ignore errors (server may have already exited)
            let _ = state.request("shutdown", serde_json::Value::Null);
            let exit_notif = LspMessage::notification("exit", serde_json::json!({}));
            let _ = state.send(&exit_notif);
            let _ = state.process.wait();
            info!(language, "LSP server stopped");
        }
        Ok(())
    }

    /// List all currently running LSP server processes.
    pub fn list_servers(&self) -> Result<Vec<LspProcess>> {
        let mut servers = self
            .servers
            .lock()
            .map_err(|_| anyhow::anyhow!("LSP proxy mutex poisoned"))?;
        let procs = servers
            .values_mut()
            .filter_map(|s| {
                if s.is_alive() {
                    Some(LspProcess {
                        language: s.language.clone(),
                        pid: s.process.id(),
                        workspace_root: s.workspace_root.clone(),
                    })
                } else {
                    None
                }
            })
            .collect();
        Ok(procs)
    }

    /// Open a file in the LSP server and collect any diagnostics published in
    /// response.
    ///
    /// Uses `textDocument/didOpen` to notify the server of the file content.
    /// The server may publish `textDocument/publishDiagnostics` notifications
    /// synchronously (e.g. rust-analyzer does this for simple errors); we
    /// collect those by draining the stdout buffer briefly.
    ///
    /// Returns an empty list if no diagnostics are published within the read
    /// window — callers should poll periodically for a complete picture.
    pub fn get_diagnostics(
        &self,
        language: &str,
        workspace_root: &Path,
        file: &Path,
        content: &str,
    ) -> Result<Vec<DiagnosticItem>> {
        let key = Self::server_key(language, workspace_root);
        let mut servers = self
            .servers
            .lock()
            .map_err(|_| anyhow::anyhow!("LSP proxy mutex poisoned"))?;
        let state = servers.get_mut(&key).ok_or_else(|| {
            anyhow::anyhow!(
                "LSP server not running for {language} at {}",
                workspace_root.display()
            )
        })?;

        let file_uri = format!("file://{}", file.display());

        // Notify the server that we have opened this file
        let lang_id = language.to_string();
        let did_open = LspMessage::notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": file_uri,
                    "languageId": lang_id,
                    "version": 1,
                    "text": content
                }
            }),
        );
        state.send(&did_open)?;

        // Drain any immediately-available messages looking for publishDiagnostics
        let mut diagnostics: Vec<DiagnosticItem> = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        // Read up to 20 messages non-blockingly to collect diagnostics
        // (We use a short timeout loop — stdin is a blocking pipe so we peek
        //  at what's already buffered)
        for _ in 0..20 {
            // Check if there's data available without blocking
            // Using fill_buf to see if anything is buffered
            let has_data = state
                .stdout
                .fill_buf()
                .map(|b| !b.is_empty())
                .unwrap_or(false);
            if !has_data {
                break;
            }

            let msg = match state.recv() {
                Ok(m) => m,
                Err(_) => break,
            };

            if msg.method.as_deref() == Some("textDocument/publishDiagnostics") {
                if let Some(params) = msg.params {
                    let items = parse_diagnostics(&file_str, &params);
                    diagnostics.extend(items);
                }
            }
        }

        Ok(diagnostics)
    }

    /// Request completions at a given cursor position in the open file.
    ///
    /// The file must have been previously opened via `get_diagnostics` or
    /// a `textDocument/didOpen` notification.
    pub fn get_completions(
        &self,
        language: &str,
        workspace_root: &Path,
        file: &Path,
        line: u32,
        col: u32,
    ) -> Result<Vec<CompletionItem>> {
        let key = Self::server_key(language, workspace_root);
        let mut servers = self
            .servers
            .lock()
            .map_err(|_| anyhow::anyhow!("LSP proxy mutex poisoned"))?;
        let state = servers.get_mut(&key).ok_or_else(|| {
            anyhow::anyhow!(
                "LSP server not running for {language} at {}",
                workspace_root.display()
            )
        })?;

        let file_uri = format!("file://{}", file.display());
        let result = state.request(
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": file_uri },
                "position": { "line": line, "character": col },
                "context": { "triggerKind": 1 }
            }),
        )?;

        Ok(parse_completions(&result))
    }
}

// ─── Parsing helpers ──────────────────────────────────────────────────────────

/// Parse a `textDocument/publishDiagnostics` params object into `DiagnosticItem`s.
fn parse_diagnostics(file_path: &str, params: &serde_json::Value) -> Vec<DiagnosticItem> {
    let diagnostics = match params.get("diagnostics").and_then(|d| d.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };
    let source_default = params
        .get("source")
        .and_then(|s| s.as_str())
        .unwrap_or("lsp");

    diagnostics
        .iter()
        .filter_map(|d| {
            let range = d.get("range")?;
            let start = range.get("start")?;
            let line = start.get("line")?.as_u64()? as u32;
            let col = start.get("character")?.as_u64()? as u32;
            let severity = d
                .get("severity")
                .and_then(|s| s.as_u64())
                .map(DiagSeverity::from_lsp_int)
                .unwrap_or(DiagSeverity::Information);
            let message = d.get("message")?.as_str()?.to_string();
            let source = d
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or(source_default)
                .to_string();
            Some(DiagnosticItem {
                file: file_path.to_string(),
                line,
                col,
                severity,
                message,
                source,
            })
        })
        .collect()
}

/// Parse a `textDocument/completion` result (list or CompletionList) into items.
fn parse_completions(result: &serde_json::Value) -> Vec<CompletionItem> {
    // Result is either CompletionList { items: [...] } or an array directly
    let items = if let Some(arr) = result.as_array() {
        arr
    } else if let Some(arr) = result.get("items").and_then(|i| i.as_array()) {
        arr
    } else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| {
            let label = item.get("label")?.as_str()?.to_string();
            let kind_int = item.get("kind").and_then(|k| k.as_u64()).unwrap_or(0);
            let kind = CompletionItem::kind_from_lsp_int(kind_int).to_string();
            let detail = item
                .get("detail")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());
            // Prefer insertText; fall back to filterText, then label
            let insert_text = item
                .get("insertText")
                .and_then(|t| t.as_str())
                .or_else(|| item.get("filterText").and_then(|t| t.as_str()))
                .unwrap_or(&label)
                .to_string();
            Some(CompletionItem {
                label,
                kind,
                detail,
                insert_text,
            })
        })
        .collect()
}
