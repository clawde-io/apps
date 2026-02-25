// SPDX-License-Identifier: MIT
/// RPC handlers for the LSP subsystem — Sprint S (LS.T01–LS.T04).
///
/// Registered methods:
///   lsp.start          — launch an LSP server for a language + workspace
///   lsp.stop           — stop a running LSP server
///   lsp.diagnostics    — get diagnostics for a file
///   lsp.completions    — get completions at a cursor position
///   lsp.listServers    — list all running LSP server processes
use crate::lsp::proxy::LspProxy;
use crate::AppContext;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::OnceLock;

/// Daemon-global LSP proxy (lazy-initialized on first use).
static LSP_PROXY: OnceLock<LspProxy> = OnceLock::new();

fn lsp_proxy() -> &'static LspProxy {
    LSP_PROXY.get_or_init(LspProxy::new)
}

// ─── Param structs ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LspStartParams {
    language: String,
    #[serde(rename = "workspaceRoot")]
    workspace_root: String,
}

#[derive(Deserialize)]
struct LspStopParams {
    language: String,
    #[serde(rename = "workspaceRoot")]
    workspace_root: String,
}

#[derive(Deserialize)]
struct LspDiagnosticsParams {
    language: String,
    #[serde(rename = "workspaceRoot")]
    workspace_root: String,
    /// Absolute path to the file to analyse.
    file: String,
    /// Current file content to send to the LSP server.
    content: String,
}

#[derive(Deserialize)]
struct LspCompletionsParams {
    language: String,
    #[serde(rename = "workspaceRoot")]
    workspace_root: String,
    /// Absolute path to the file.
    file: String,
    /// 0-based line number.
    line: u32,
    /// 0-based column number.
    col: u32,
}

// ─── Path validation ──────────────────────────────────────────────────────────

fn validate_abs_path(path: &str, name: &str) -> Result<()> {
    if path.contains('\0') {
        bail!("invalid {name}: null byte");
    }
    if !Path::new(path).is_absolute() {
        bail!("invalid {name}: must be an absolute path");
    }
    Ok(())
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// `lsp.start` — spawn (or reconnect to) the LSP server for `language` at
/// `workspaceRoot`, perform the LSP `initialize` handshake, and return the
/// process id and language.
pub async fn lsp_start(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: LspStartParams = serde_json::from_value(params)?;
    validate_abs_path(&p.workspace_root, "workspaceRoot")?;

    let proxy = lsp_proxy();
    let language = p.language.clone();
    let workspace_root = p.workspace_root.clone();

    // Spawning a subprocess is blocking I/O — offload to the thread pool.
    let process = tokio::task::spawn_blocking(move || {
        proxy.start_server(&language, Path::new(&workspace_root))
    })
    .await??;

    Ok(json!({
        "language": process.language,
        "pid": process.pid,
        "workspaceRoot": process.workspace_root
    }))
}

/// `lsp.stop` — send the LSP `shutdown` + `exit` sequence and remove the
/// process from the pool.
pub async fn lsp_stop(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: LspStopParams = serde_json::from_value(params)?;
    validate_abs_path(&p.workspace_root, "workspaceRoot")?;

    let proxy = lsp_proxy();
    let language = p.language.clone();
    let workspace_root = p.workspace_root.clone();

    tokio::task::spawn_blocking(move || {
        proxy.stop_server(&language, Path::new(&workspace_root))
    })
    .await??;

    Ok(json!({ "stopped": true, "language": p.language }))
}

/// `lsp.diagnostics` — open the file in the LSP server and return any
/// diagnostics that are published synchronously.
///
/// The LSP server must have been started via `lsp.start` first.
pub async fn lsp_diagnostics(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: LspDiagnosticsParams = serde_json::from_value(params)?;
    validate_abs_path(&p.workspace_root, "workspaceRoot")?;
    validate_abs_path(&p.file, "file")?;

    let proxy = lsp_proxy();
    let language = p.language.clone();
    let workspace_root = p.workspace_root.clone();
    let file = p.file.clone();
    let content = p.content.clone();

    let items = tokio::task::spawn_blocking(move || {
        proxy.get_diagnostics(
            &language,
            Path::new(&workspace_root),
            Path::new(&file),
            &content,
        )
    })
    .await??;

    Ok(json!({ "diagnostics": items }))
}

/// `lsp.completions` — request completions at the given cursor position.
///
/// The file must have been opened previously (via `lsp.diagnostics` or
/// a prior `lsp.completions` call on the same file).
pub async fn lsp_completions(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: LspCompletionsParams = serde_json::from_value(params)?;
    validate_abs_path(&p.workspace_root, "workspaceRoot")?;
    validate_abs_path(&p.file, "file")?;

    let proxy = lsp_proxy();
    let language = p.language.clone();
    let workspace_root = p.workspace_root.clone();
    let file = p.file.clone();
    let line = p.line;
    let col = p.col;

    let items = tokio::task::spawn_blocking(move || {
        proxy.get_completions(
            &language,
            Path::new(&workspace_root),
            Path::new(&file),
            line,
            col,
        )
    })
    .await??;

    Ok(json!({ "completions": items }))
}

/// `lsp.listServers` — return a list of all currently running LSP server
/// processes managed by this daemon instance.
pub async fn lsp_list_servers(_params: Value, ctx: &AppContext) -> Result<Value> {
    let proxy = lsp_proxy();

    let servers = tokio::task::spawn_blocking(move || proxy.list_servers()).await??;

    Ok(json!({ "servers": servers }))
}
