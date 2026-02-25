// SPDX-License-Identifier: MIT
/// VS Code settings bridge — Sprint S, LS.T05 / LS.T06.
///
/// Reads VS Code's `settings.json` from the standard platform locations and
/// maps common editor settings to their ClawDE / daemon equivalents.
///
/// ## Mapped settings
///
/// | VS Code key                | ClawDE equivalent           |
/// |----------------------------|-----------------------------|
/// | `editor.fontSize`          | `formatting.font_size`      |
/// | `editor.fontFamily`        | `formatting.font_family`    |
/// | `editor.tabSize`           | `formatting.indent_size`    |
/// | `editor.insertSpaces`      | `formatting.use_spaces`     |
/// | `editor.wordWrap`          | `formatting.word_wrap`      |
/// | `workbench.colorTheme`     | `ui.theme`                  |
/// | `editor.lineNumbers`       | `ui.line_numbers`           |
/// | `editor.minimap.enabled`   | `ui.minimap`                |
/// | `editor.rulers`            | `formatting.rulers`         |
/// | `files.trimTrailingWhitespace` | `formatting.trim_trailing_whitespace` |
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tracing::debug;

// ─── Platform paths ───────────────────────────────────────────────────────────

/// Return the platform-specific path to the VS Code user-level `settings.json`.
///
/// - macOS: `~/Library/Application Support/Code/User/settings.json`
/// - Linux: `~/.config/Code/User/settings.json`
/// - Windows: `%APPDATA%\Code\User\settings.json`
pub fn vscode_user_settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").ok()?;
        Some(
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Code")
                .join("User")
                .join("settings.json"),
        )
    }
    #[cfg(target_os = "linux")]
    {
        let config = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".config"))
            })?;
        Some(config.join("Code").join("User").join("settings.json"))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").ok()?;
        Some(
            PathBuf::from(appdata)
                .join("Code")
                .join("User")
                .join("settings.json"),
        )
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Return the path to the VS Code *workspace-level* `settings.json`.
///
/// Workspace settings live at `{workspace}/.vscode/settings.json` and override
/// the user-level settings for the workspace.
pub fn vscode_workspace_settings_path(workspace: &Path) -> PathBuf {
    workspace.join(".vscode").join("settings.json")
}

// ─── Reading ──────────────────────────────────────────────────────────────────

/// Read a VS Code `settings.json` file and return the parsed JSON value.
///
/// VS Code settings files use the JSONC (JSON with comments) format.  This
/// function strips `//` line comments before parsing, matching the behaviour
/// of the extension_host module's `strip_line_comments`.
pub fn read_vscode_settings(workspace: &Path) -> Result<Value> {
    // Prefer workspace-level settings; fall back to user-level.
    let ws_path = vscode_workspace_settings_path(workspace);
    let path = if ws_path.exists() {
        ws_path
    } else if let Some(user_path) = vscode_user_settings_path() {
        if user_path.exists() {
            user_path
        } else {
            debug!("no VS Code settings.json found");
            return Ok(Value::Object(Default::default()));
        }
    } else {
        return Ok(Value::Object(Default::default()));
    };

    debug!(path = %path.display(), "reading VS Code settings");
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("could not read VS Code settings at {}", path.display()))?;

    let cleaned = crate::vscode::strip_jsonc_comments(&content);
    serde_json::from_str(&cleaned)
        .with_context(|| format!("could not parse VS Code settings at {}", path.display()))
}

// ─── Conversion ───────────────────────────────────────────────────────────────

/// A flat representation of the daemon/ClawDE formatting and UI configuration
/// that can be derived from VS Code settings.
///
/// All fields are optional — only settings that are present in the VS Code
/// config are populated.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DaemonConfig {
    // ── Formatting ───────────────────────────────────────────────────────────
    /// Font size in points (maps from `editor.fontSize`).
    pub font_size: Option<u32>,
    /// Font family string (maps from `editor.fontFamily`).
    pub font_family: Option<String>,
    /// Number of spaces per indent level (maps from `editor.tabSize`).
    pub indent_size: Option<u32>,
    /// Whether to use spaces instead of tabs (maps from `editor.insertSpaces`).
    pub use_spaces: Option<bool>,
    /// Word wrap setting: `"on"`, `"off"`, `"wordWrapColumn"`, `"bounded"`
    /// (maps from `editor.wordWrap`).
    pub word_wrap: Option<String>,
    /// Column positions for vertical rulers (maps from `editor.rulers`).
    pub rulers: Option<Vec<u32>>,
    /// Whether to trim trailing whitespace on save
    /// (maps from `files.trimTrailingWhitespace`).
    pub trim_trailing_whitespace: Option<bool>,
    // ── UI ───────────────────────────────────────────────────────────────────
    /// Theme name (maps from `workbench.colorTheme`).
    pub theme: Option<String>,
    /// Line number display mode: `"on"`, `"off"`, `"relative"`, `"interval"`
    /// (maps from `editor.lineNumbers`).
    pub line_numbers: Option<String>,
    /// Whether to show the minimap (maps from `editor.minimap.enabled`).
    pub minimap: Option<bool>,
}

/// Map a parsed VS Code `settings.json` JSON object to a `DaemonConfig`.
///
/// Unknown keys are silently ignored — this is a one-way, best-effort mapping.
pub fn convert_to_clawd_config(vscode_settings: &Value) -> DaemonConfig {
    let obj = match vscode_settings.as_object() {
        Some(o) => o,
        None => return DaemonConfig::default(),
    };

    let mut cfg = DaemonConfig::default();

    // ── editor.fontSize ──────────────────────────────────────────────────────
    if let Some(v) = obj.get("editor.fontSize").and_then(|v| v.as_f64()) {
        cfg.font_size = Some(v.round() as u32);
    }

    // ── editor.fontFamily ────────────────────────────────────────────────────
    if let Some(v) = obj.get("editor.fontFamily").and_then(|v| v.as_str()) {
        cfg.font_family = Some(v.trim().to_string());
    }

    // ── editor.tabSize ───────────────────────────────────────────────────────
    if let Some(v) = obj.get("editor.tabSize").and_then(|v| v.as_u64()) {
        cfg.indent_size = Some(v as u32);
    }

    // ── editor.insertSpaces ──────────────────────────────────────────────────
    if let Some(v) = obj.get("editor.insertSpaces").and_then(|v| v.as_bool()) {
        cfg.use_spaces = Some(v);
    }

    // ── editor.wordWrap ──────────────────────────────────────────────────────
    if let Some(v) = obj.get("editor.wordWrap").and_then(|v| v.as_str()) {
        cfg.word_wrap = Some(v.to_string());
    }

    // ── editor.rulers ────────────────────────────────────────────────────────
    if let Some(arr) = obj.get("editor.rulers").and_then(|v| v.as_array()) {
        let rulers: Vec<u32> = arr
            .iter()
            .filter_map(|v| {
                // Can be a plain number or { column: N, color: "..." }
                v.as_u64()
                    .map(|n| n as u32)
                    .or_else(|| {
                        v.get("column")
                            .and_then(|c| c.as_u64())
                            .map(|n| n as u32)
                    })
            })
            .collect();
        if !rulers.is_empty() {
            cfg.rulers = Some(rulers);
        }
    }

    // ── files.trimTrailingWhitespace ─────────────────────────────────────────
    if let Some(v) = obj
        .get("files.trimTrailingWhitespace")
        .and_then(|v| v.as_bool())
    {
        cfg.trim_trailing_whitespace = Some(v);
    }

    // ── workbench.colorTheme ──────────────────────────────────────────────────
    if let Some(v) = obj.get("workbench.colorTheme").and_then(|v| v.as_str()) {
        cfg.theme = Some(v.to_string());
    }

    // ── editor.lineNumbers ───────────────────────────────────────────────────
    if let Some(v) = obj.get("editor.lineNumbers").and_then(|v| v.as_str()) {
        cfg.line_numbers = Some(v.to_string());
    }

    // ── editor.minimap.enabled ───────────────────────────────────────────────
    if let Some(minimap_obj) = obj.get("editor.minimap") {
        if let Some(enabled) = minimap_obj.get("enabled").and_then(|v| v.as_bool()) {
            cfg.minimap = Some(enabled);
        }
    }
    // Also handle the flat key form used in workspace settings
    if let Some(v) = obj
        .get("editor.minimap.enabled")
        .and_then(|v| v.as_bool())
    {
        cfg.minimap = Some(v);
    }

    cfg
}
