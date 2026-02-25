// SPDX-License-Identifier: MIT
//! Editor context types — Sprint Z, IE.T01–IE.T08.
//!
//! Represents the state of an IDE's active editor window as reported by a
//! connected IDE extension (VS Code, JetBrains, Neovim, Emacs).  The daemon
//! holds the most-recent context per connected extension and exposes it to
//! sessions so the AI can be aware of the developer's current focus.

use serde::{Deserialize, Serialize};

/// The type of IDE extension that reported this context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionType {
    Vscode,
    Jetbrains,
    Neovim,
    Emacs,
    /// Unknown / future extension type — stored as-is.
    #[serde(untagged)]
    Other(String),
}

impl ExtensionType {
    /// Parse from a raw string such as `"vscode"`.
    pub fn from_str(s: &str) -> Self {
        match s {
            "vscode" => Self::Vscode,
            "jetbrains" => Self::Jetbrains,
            "neovim" => Self::Neovim,
            "emacs" => Self::Emacs,
            other => Self::Other(other.to_string()),
        }
    }

    /// Return the string representation for storage and display.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Vscode => "vscode",
            Self::Jetbrains => "jetbrains",
            Self::Neovim => "neovim",
            Self::Emacs => "emacs",
            Self::Other(s) => s.as_str(),
        }
    }
}

/// Current editor state as reported by a connected IDE extension.
///
/// All fields except `extension_type` and `updated_at` are optional — an
/// extension may omit any field it cannot determine (e.g. Neovim may not
/// report `workspace_root`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorContext {
    /// Which IDE reported this context: `"vscode"` | `"jetbrains"` | `"neovim"` | `"emacs"`.
    pub extension_type: String,
    /// Absolute path of the file currently open in the active editor tab.
    pub file_path: Option<String>,
    /// Language identifier (VS Code language ID, e.g. `"rust"`, `"typescript"`).
    pub language: Option<String>,
    /// 0-based line number of the primary cursor position.
    pub cursor_line: Option<u32>,
    /// 0-based column number of the primary cursor position.
    pub cursor_col: Option<u32>,
    /// Text currently selected in the editor, if any.
    pub selection_text: Option<String>,
    /// First line of the visible range (inclusive, 0-based).
    pub visible_range_start: Option<u32>,
    /// Last line of the visible range (inclusive, 0-based).
    pub visible_range_end: Option<u32>,
    /// Absolute path of the workspace / project root folder.
    pub workspace_root: Option<String>,
    /// ISO-8601 UTC timestamp of when this context was last updated.
    pub updated_at: String,
}

impl EditorContext {
    /// Construct a minimal context with the mandatory fields filled in.
    pub fn new(extension_type: impl Into<String>) -> Self {
        Self {
            extension_type: extension_type.into(),
            file_path: None,
            language: None,
            cursor_line: None,
            cursor_col: None,
            selection_text: None,
            visible_range_start: None,
            visible_range_end: None,
            workspace_root: None,
            updated_at: crate::ide::now_utc(),
        }
    }
}

/// A record of an IDE extension that has connected to this daemon instance.
///
/// Stored in the `ide_connections` table and used to broadcast `settings.changed`
/// events back to the correct extension type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeConnectionRecord {
    /// Unique connection ID (UUID).
    pub connection_id: String,
    /// Extension type string.
    pub extension_type: String,
    /// Extension version string as reported by the extension (e.g. `"1.2.3"`).
    pub extension_version: Option<String>,
    /// ISO-8601 UTC timestamp when the extension first connected.
    pub connected_at: String,
    /// ISO-8601 UTC timestamp of the last keep-alive or context update.
    pub last_seen_at: String,
}
