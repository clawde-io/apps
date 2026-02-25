// SPDX-License-Identifier: MIT
/// LSP data model — Sprint S (LS.T01–LS.T04).
///
/// These types mirror the Language Server Protocol 3.17 wire format closely
/// enough to parse real LSP server responses, while staying lightweight for
/// the daemon's internal use.
use serde::{Deserialize, Serialize};

// ─── Configuration ────────────────────────────────────────────────────────────

/// Per-language LSP server configuration.
///
/// Stored in the daemon config and keyed by language name (e.g. `"rust"`,
/// `"typescript"`, `"dart"`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspConfig {
    /// Human-readable language name (e.g. `"rust"`, `"typescript"`, `"dart"`).
    pub language: String,
    /// The executable to launch (e.g. `"rust-analyzer"`, `"typescript-language-server"`).
    pub server_command: Vec<String>,
    /// Extra command-line arguments passed after the executable name.
    pub server_args: Vec<String>,
    /// File extensions this server handles (e.g. `[".rs"]`, `[".ts", ".tsx"]`).
    pub file_extensions: Vec<String>,
}

impl LspConfig {
    /// Returns built-in configs for the language servers ClawDE supports out-of-the-box.
    ///
    /// Users can override these via the daemon config file.
    pub fn builtin_defaults() -> Vec<LspConfig> {
        vec![
            LspConfig {
                language: "rust".into(),
                server_command: vec!["rust-analyzer".into()],
                server_args: vec![],
                file_extensions: vec![".rs".into()],
            },
            LspConfig {
                language: "typescript".into(),
                server_command: vec!["typescript-language-server".into()],
                server_args: vec!["--stdio".into()],
                file_extensions: vec![".ts".into(), ".tsx".into()],
            },
            LspConfig {
                language: "javascript".into(),
                server_command: vec!["typescript-language-server".into()],
                server_args: vec!["--stdio".into()],
                file_extensions: vec![".js".into(), ".jsx".into(), ".mjs".into()],
            },
            LspConfig {
                language: "dart".into(),
                server_command: vec!["dart".into()],
                server_args: vec!["language-server".into(), "--protocol=lsp".into()],
                file_extensions: vec![".dart".into()],
            },
            LspConfig {
                language: "go".into(),
                server_command: vec!["gopls".into()],
                server_args: vec![],
                file_extensions: vec![".go".into()],
            },
            LspConfig {
                language: "python".into(),
                server_command: vec!["pylsp".into()],
                server_args: vec![],
                file_extensions: vec![".py".into()],
            },
        ]
    }

    /// Detect which language server config applies to a file by its extension.
    pub fn for_extension<'a>(configs: &'a [LspConfig], ext: &str) -> Option<&'a LspConfig> {
        configs
            .iter()
            .find(|c| c.file_extensions.iter().any(|e| e.as_str() == ext))
    }
}

// ─── Running process ──────────────────────────────────────────────────────────

/// A live LSP server process tracked by the `LspProxy`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspProcess {
    /// Language name this process serves.
    pub language: String,
    /// OS process ID.
    pub pid: u32,
    /// Absolute path to the workspace root this server was launched for.
    pub workspace_root: String,
}

// ─── Diagnostics ─────────────────────────────────────────────────────────────

/// Severity levels matching LSP `DiagnosticSeverity` (1-based in LSP spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagSeverity {
    /// Parse from an LSP integer value (1 = error, 2 = warning, 3 = info, 4 = hint).
    pub fn from_lsp_int(n: u64) -> Self {
        match n {
            1 => DiagSeverity::Error,
            2 => DiagSeverity::Warning,
            3 => DiagSeverity::Information,
            4 => DiagSeverity::Hint,
            _ => DiagSeverity::Information,
        }
    }
}

/// A single diagnostic finding returned by an LSP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticItem {
    /// Absolute file path.
    pub file: String,
    /// 0-based line number (same as LSP).
    pub line: u32,
    /// 0-based column number.
    pub col: u32,
    /// Severity level.
    pub severity: DiagSeverity,
    /// Human-readable message.
    pub message: String,
    /// Source tool name (e.g. `"rust-analyzer"`, `"eslint"`).
    pub source: String,
}

// ─── Completions ──────────────────────────────────────────────────────────────

/// A single LSP completion item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    /// The display label shown in the completion list.
    pub label: String,
    /// Kind string (e.g. `"function"`, `"variable"`, `"class"`, `"module"`).
    pub kind: String,
    /// Optional detail string (e.g. type signature).
    pub detail: Option<String>,
    /// Text to insert when the user accepts this completion.
    pub insert_text: String,
}

impl CompletionItem {
    /// Map LSP `completionItemKind` integer to a human-readable kind string.
    ///
    /// LSP 3.17 §3.18.8 — only common values are mapped; all others fall back to "value".
    pub fn kind_from_lsp_int(kind: u64) -> &'static str {
        match kind {
            1 => "text",
            2 => "method",
            3 => "function",
            4 => "constructor",
            5 => "field",
            6 => "variable",
            7 => "class",
            8 => "interface",
            9 => "module",
            10 => "property",
            11 => "unit",
            12 => "value",
            13 => "enum",
            14 => "keyword",
            15 => "snippet",
            16 => "color",
            17 => "file",
            18 => "reference",
            19 => "folder",
            20 => "enum_member",
            21 => "constant",
            22 => "struct",
            23 => "event",
            24 => "operator",
            25 => "type_parameter",
            _ => "value",
        }
    }
}

// ─── LSP wire types (used for parsing server responses) ──────────────────────

/// Minimal JSON-RPC 2.0 message sent to / received from an LSP server via stdio.
#[derive(Debug, Serialize, Deserialize)]
pub struct LspMessage {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
}

impl LspMessage {
    /// Build a JSON-RPC request with a numeric id.
    pub fn request(id: u64, method: &str, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(id)),
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Build a JSON-RPC notification (no `id`).
    pub fn notification(method: &str, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id: None,
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── LspConfig ──────────────────────────────────────────────────────────

    #[test]
    fn builtin_defaults_returns_all_six_languages() {
        let defaults = LspConfig::builtin_defaults();
        let languages: Vec<&str> = defaults.iter().map(|c| c.language.as_str()).collect();
        assert!(languages.contains(&"rust"));
        assert!(languages.contains(&"typescript"));
        assert!(languages.contains(&"javascript"));
        assert!(languages.contains(&"dart"));
        assert!(languages.contains(&"go"));
        assert!(languages.contains(&"python"));
        assert_eq!(defaults.len(), 6);
    }

    #[test]
    fn for_extension_finds_rust() {
        let configs = LspConfig::builtin_defaults();
        let cfg = LspConfig::for_extension(&configs, ".rs").expect("should find .rs config");
        assert_eq!(cfg.language, "rust");
        assert_eq!(cfg.server_command, vec!["rust-analyzer"]);
    }

    #[test]
    fn for_extension_finds_typescript() {
        let configs = LspConfig::builtin_defaults();
        let cfg = LspConfig::for_extension(&configs, ".tsx").expect("should find .tsx config");
        assert_eq!(cfg.language, "typescript");
    }

    #[test]
    fn for_extension_returns_none_for_unknown() {
        let configs = LspConfig::builtin_defaults();
        assert!(LspConfig::for_extension(&configs, ".java").is_none());
    }

    // ── DiagSeverity ───────────────────────────────────────────────────────

    #[test]
    fn diag_severity_from_lsp_int_maps_correctly() {
        assert_eq!(DiagSeverity::from_lsp_int(1), DiagSeverity::Error);
        assert_eq!(DiagSeverity::from_lsp_int(2), DiagSeverity::Warning);
        assert_eq!(DiagSeverity::from_lsp_int(3), DiagSeverity::Information);
        assert_eq!(DiagSeverity::from_lsp_int(4), DiagSeverity::Hint);
    }

    #[test]
    fn diag_severity_unknown_int_falls_back_to_info() {
        assert_eq!(DiagSeverity::from_lsp_int(99), DiagSeverity::Information);
        assert_eq!(DiagSeverity::from_lsp_int(0), DiagSeverity::Information);
    }

    // ── CompletionItem ────────────────────────────────────────────────────

    #[test]
    fn completion_kind_maps_common_values() {
        assert_eq!(CompletionItem::kind_from_lsp_int(2), "method");
        assert_eq!(CompletionItem::kind_from_lsp_int(3), "function");
        assert_eq!(CompletionItem::kind_from_lsp_int(6), "variable");
        assert_eq!(CompletionItem::kind_from_lsp_int(7), "class");
        assert_eq!(CompletionItem::kind_from_lsp_int(14), "keyword");
    }

    #[test]
    fn completion_kind_unknown_falls_back_to_value() {
        assert_eq!(CompletionItem::kind_from_lsp_int(0), "value");
        assert_eq!(CompletionItem::kind_from_lsp_int(999), "value");
    }

    // ── LspMessage ────────────────────────────────────────────────────────

    #[test]
    fn lsp_message_request_sets_fields() {
        let msg = LspMessage::request(42, "textDocument/hover", serde_json::json!({}));
        assert_eq!(msg.jsonrpc, "2.0");
        assert_eq!(msg.method.as_deref(), Some("textDocument/hover"));
        assert!(msg.id.is_some());
        assert!(msg.result.is_none());
        assert!(msg.error.is_none());
    }

    #[test]
    fn lsp_message_notification_has_no_id() {
        let msg = LspMessage::notification("textDocument/didOpen", serde_json::json!({}));
        assert_eq!(msg.jsonrpc, "2.0");
        assert!(msg.id.is_none());
        assert_eq!(msg.method.as_deref(), Some("textDocument/didOpen"));
    }

    #[test]
    fn lsp_message_roundtrip_json() {
        let msg = LspMessage::request(1, "initialize", serde_json::json!({ "rootUri": "file:///tmp" }));
        let json = serde_json::to_string(&msg).unwrap();
        let back: LspMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.method.as_deref(), Some("initialize"));
    }
}
