// SPDX-License-Identifier: MIT
//! Data models for Builder Mode.

use serde::{Deserialize, Serialize};

/// Current phase of a builder session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderStatus {
    /// AI is generating the plan (file list, package choices, etc.).
    Planning,
    /// Files are being written to disk.
    Building,
    /// Scaffold complete — ready to open in a session.
    Done,
    /// Something went wrong; see `error` field on `BuilderSession`.
    Failed,
}

/// An in-progress (or completed) builder session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderSession {
    /// Unique session identifier (UUID v4).
    pub id: String,
    /// Short name of the selected stack template (e.g. `"react-vite"`).
    pub target_stack: String,
    /// Human-readable name of the template.
    pub template_name: String,
    /// Current lifecycle phase.
    pub status: BuilderStatus,
    /// Absolute path where files are being written.
    pub output_dir: String,
    /// User-supplied description of what they want to build.
    pub description: String,
    /// Files written so far (relative to `output_dir`).
    pub files_written: Vec<String>,
    /// Optional error message when `status == Failed`.
    pub error: Option<String>,
    /// RFC-3339 creation timestamp.
    pub created_at: String,
}

/// A single file that a `StackTemplate` will generate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateFile {
    /// Path relative to the project root (e.g. `"src/App.tsx"`).
    pub path: String,
    /// Full file content to write verbatim.
    pub content: String,
}

/// A named stack template that Builder Mode can scaffold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackTemplate {
    /// Machine-readable key (matches `BuilderSession::target_stack`).
    pub name: String,
    /// One-line description shown in the picker.
    pub description: String,
    /// All files the template will generate.
    pub files: Vec<TemplateFile>,
}
