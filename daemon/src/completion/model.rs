// SPDX-License-Identifier: MIT
// Code Completion Engine — data model (Sprint K, CC.T01).

use serde::{Deserialize, Serialize};

/// Input parameters for a fill-in-middle completion request.
///
/// The cursor position splits the file content into a *prefix* (everything
/// before the cursor) and a *suffix* (everything after the cursor).  The
/// provider is asked to generate the missing text that fits between them.
#[derive(Debug, Clone, Deserialize)]
pub struct CompletionRequest {
    /// Session ID to route the completion through.
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Absolute path of the file being edited (for language detection).
    #[serde(rename = "filePath")]
    pub file_path: String,
    /// 0-based line number of the cursor position.
    #[serde(rename = "cursorLine")]
    pub cursor_line: usize,
    /// 0-based column offset of the cursor position.
    #[serde(rename = "cursorCol")]
    pub cursor_col: usize,
    /// Full content of the file being edited.
    #[serde(rename = "fileContent")]
    pub file_content: String,
    /// Text immediately before the cursor (end of prefix).
    pub prefix: String,
    /// Text immediately after the cursor (start of suffix).
    pub suffix: String,
}

/// A single code completion suggestion returned by the provider.
#[derive(Debug, Clone, Serialize)]
pub struct CompletionSuggestion {
    /// The suggested code text to insert at the cursor position.
    pub text: String,
    /// 0-based start line of the suggestion range (inclusive).
    pub start_line: usize,
    /// 0-based end line of the suggestion range (inclusive).
    pub end_line: usize,
}
