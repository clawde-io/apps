// SPDX-License-Identifier: MIT
//! RPC handlers for IDE extension integration — Sprint Z, IE.T05–IE.T08.
//!
//! Exposes three RPC methods:
//!
//! | Method                    | Direction    | Description                                   |
//! |---------------------------|-------------|-----------------------------------------------|
//! | `ide.extensionConnected`  | IDE → daemon | Register that an extension has connected       |
//! | `ide.editorContext`       | IDE → daemon | Push current editor state into the daemon      |
//! | `ide.syncSettings`        | app → daemon | Broadcast settings to all connected extensions |
//!
//! These handlers are wired into the dispatch table in `ipc/mod.rs` under the
//! `ide.*` namespace.  See `sprint_Z_wiring_notes.md` for the exact lines to add.

use crate::ide::editor_context::EditorContext;
use crate::AppContext;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::warn;

// ─── ide.extensionConnected ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct ExtensionConnectedParams {
    /// Extension type — `"vscode"` | `"jetbrains"` | `"neovim"` | `"emacs"`.
    #[serde(rename = "extensionType")]
    extension_type: String,
    /// Optional version string as reported by the extension manifest.
    #[serde(rename = "extensionVersion")]
    extension_version: Option<String>,
}

/// `ide.extensionConnected` — record that an IDE extension has connected.
///
/// Params:
/// ```json
/// { "extensionType": "vscode", "extensionVersion": "1.2.3" }
/// ```
/// Returns: `{ "connectionId": "<uuid>" }`
pub async fn extension_connected(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: ExtensionConnectedParams = serde_json::from_value(params)
        .map_err(|e| anyhow::anyhow!("invalid params for ide.extensionConnected: {}", e))?;

    let connection_id = {
        let mut bridge = ctx.ide_bridge.write().await;
        bridge.register_connection(&p.extension_type, p.extension_version.as_deref())
    };

    ctx.broadcaster.broadcast(
        "ide.extensionConnected",
        json!({
            "connectionId": connection_id,
            "extensionType": p.extension_type,
            "extensionVersion": p.extension_version,
        }),
    );

    Ok(json!({ "connectionId": connection_id }))
}

// ─── ide.editorContext ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct EditorContextParams {
    /// Connection ID returned by `ide.extensionConnected`.
    #[serde(rename = "connectionId")]
    connection_id: String,
    /// Which IDE this came from.
    #[serde(rename = "extensionType")]
    extension_type: String,
    /// Absolute path of the currently open file.
    #[serde(rename = "filePath")]
    file_path: Option<String>,
    /// VS Code language identifier (e.g. `"rust"`, `"typescript"`).
    language: Option<String>,
    /// 0-based cursor line.
    #[serde(rename = "cursorLine")]
    cursor_line: Option<u32>,
    /// 0-based cursor column.
    #[serde(rename = "cursorCol")]
    cursor_col: Option<u32>,
    /// Currently selected text.
    #[serde(rename = "selectionText")]
    selection_text: Option<String>,
    /// First visible line (0-based, inclusive).
    #[serde(rename = "visibleRangeStart")]
    visible_range_start: Option<u32>,
    /// Last visible line (0-based, inclusive).
    #[serde(rename = "visibleRangeEnd")]
    visible_range_end: Option<u32>,
    /// Absolute path of the workspace root.
    #[serde(rename = "workspaceRoot")]
    workspace_root: Option<String>,
}

/// `ide.editorContext` — receive and store editor context from a connected IDE.
///
/// Stores the context in the in-memory bridge and broadcasts an
/// `editor.contextChanged` push event to all other connected clients
/// (e.g. the Flutter desktop app can display "currently editing foo.rs").
///
/// Params: see [`EditorContextParams`].
/// Returns: `{ "stored": true }`
pub async fn editor_context(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: EditorContextParams = serde_json::from_value(params)
        .map_err(|e| anyhow::anyhow!("invalid params for ide.editorContext: {}", e))?;

    let editor_ctx = EditorContext {
        extension_type: p.extension_type.clone(),
        file_path: p.file_path.clone(),
        language: p.language.clone(),
        cursor_line: p.cursor_line,
        cursor_col: p.cursor_col,
        selection_text: p.selection_text.clone(),
        visible_range_start: p.visible_range_start,
        visible_range_end: p.visible_range_end,
        workspace_root: p.workspace_root.clone(),
        updated_at: crate::ide::now_utc(),
    };

    {
        let mut bridge = ctx.ide_bridge.write().await;
        bridge.update_context(&p.connection_id, editor_ctx.clone());
    }

    ctx.broadcaster.broadcast(
        "editor.contextChanged",
        json!({
            "connectionId": p.connection_id,
            "extensionType": p.extension_type,
            "filePath": p.file_path,
            "language": p.language,
            "cursorLine": p.cursor_line,
            "cursorCol": p.cursor_col,
            "selectionText": p.selection_text,
            "visibleRangeStart": p.visible_range_start,
            "visibleRangeEnd": p.visible_range_end,
            "workspaceRoot": p.workspace_root,
            "updatedAt": editor_ctx.updated_at,
        }),
    );

    Ok(json!({ "stored": true }))
}

// ─── ide.syncSettings ─────────────────────────────────────────────────────────

/// `ide.syncSettings` — push daemon/app settings out to all connected IDE extensions.
///
/// Called by the Flutter desktop app when the user saves settings.  The daemon
/// broadcasts a `settings.changed` push event carrying the settings payload so
/// connected extensions can react (e.g. update their status bar, adjust
/// inline-suggestion behaviour).
///
/// Params: any JSON object representing the settings diff or full settings blob.
/// Returns: `{ "broadcast": true, "extensionCount": N }`
pub async fn sync_settings(params: Value, ctx: &AppContext) -> Result<Value> {
    let extension_count = {
        let bridge = ctx.ide_bridge.read().await;
        bridge.connection_count()
    };

    if extension_count == 0 {
        // No extensions connected — still return success; the caller doesn't
        // need to know whether any extension is listening.
        return Ok(json!({ "broadcast": true, "extensionCount": 0 }));
    }

    // Validate that params is an object (settings must be a key/value map).
    if !params.is_object() && !params.is_null() {
        warn!("ide.syncSettings: params is not an object — broadcasting anyway");
    }

    ctx.broadcaster.broadcast("settings.changed", params);

    Ok(json!({ "broadcast": true, "extensionCount": extension_count }))
}

// ─── ide.listConnections ──────────────────────────────────────────────────────

/// `ide.listConnections` — list all currently-connected IDE extensions.
///
/// Useful for the desktop app to display an "IDE connections" panel.
///
/// Params: (none)
/// Returns: `{ "connections": [...], "count": N }`
pub async fn list_connections(_params: Value, ctx: &AppContext) -> Result<Value> {
    let bridge = ctx.ide_bridge.read().await;
    let connections: Vec<Value> = bridge
        .list_connections()
        .into_iter()
        .map(|c| {
            json!({
                "connectionId": c.connection_id,
                "extensionType": c.extension_type,
                "extensionVersion": c.extension_version,
                "connectedAt": c.connected_at,
                "lastSeenAt": c.last_seen_at,
            })
        })
        .collect();

    let count = connections.len();
    Ok(json!({ "connections": connections, "count": count }))
}

// ─── ide.latestContext ────────────────────────────────────────────────────────

/// `ide.latestContext` — return the most-recent editor context from any connected IDE.
///
/// Params: (none)
/// Returns: the [`EditorContext`] JSON, or `null` if no IDE is connected.
pub async fn latest_context(_params: Value, ctx: &AppContext) -> Result<Value> {
    let bridge = ctx.ide_bridge.read().await;
    match bridge.latest_context() {
        Some(ec) => Ok(serde_json::to_value(ec)?),
        None => Ok(Value::Null),
    }
}
