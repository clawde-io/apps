//! Thread and turn data models for conversation threading (Phase 43f).
//!
//! These types are stored in SQLite and serialized over the RPC wire.
//! All IDs use the `TH-{uuid8}` format for threads and UUID v4 for turns/items.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The role a thread plays in conversation flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreadType {
    /// Persistent per-project orchestrator — never edits files, only creates
    /// tasks, shows status, and requests approvals.
    Control,
    /// Scoped to one task, runs in a git worktree, can call file-write tools.
    Task,
    /// Forked from a task thread for parallel exploration.
    Sub,
}

impl ThreadType {
    /// Return the canonical SQL string stored in `threads.thread_type`.
    pub fn as_str(&self) -> &'static str {
        match self {
            ThreadType::Control => "control",
            ThreadType::Task => "task",
            ThreadType::Sub => "sub",
        }
    }
}

impl std::fmt::Display for ThreadType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Lifecycle state of a thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreadStatus {
    Active,
    Paused,
    Completed,
    Archived,
    Error,
}

impl ThreadStatus {
    /// Return the canonical SQL string stored in `threads.status`.
    pub fn as_str(&self) -> &'static str {
        match self {
            ThreadStatus::Active => "active",
            ThreadStatus::Paused => "paused",
            ThreadStatus::Completed => "completed",
            ThreadStatus::Archived => "archived",
            ThreadStatus::Error => "error",
        }
    }
}

impl std::fmt::Display for ThreadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A conversation thread — either a persistent control thread or a
/// task-scoped thread that runs in a git worktree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub thread_id: String,
    pub thread_type: ThreadType,
    /// Set for Task and Sub threads; `None` for Control threads.
    pub task_id: Option<String>,
    /// Set for Sub threads forked from a parent; `None` for root threads.
    pub parent_thread_id: Option<String>,
    pub status: ThreadStatus,
    /// JSON object: `{ "provider": "claude", "model": "claude-sonnet-4-5", "max_tokens": 8192 }`
    pub model_config: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single conversational turn within a thread.
/// One turn = one request + one response (or a standalone user message).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub turn_id: String,
    pub thread_id: String,
    /// `"user"` | `"assistant"` | `"system"` | `"tool"`
    pub role: String,
    /// Text content of the turn (may be empty when `tool_calls` carries the payload).
    pub content: String,
    /// JSON array of OpenAI-compatible tool call objects.
    pub tool_calls: Vec<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// A sub-item within a turn — used to store structured content blocks
/// (text chunks, individual tool calls, tool results, images).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub item_id: String,
    pub turn_id: String,
    /// `"text"` | `"tool_call"` | `"tool_result"` | `"image"`
    pub item_type: String,
    /// Structured content; shape depends on `item_type`.
    pub content: serde_json::Value,
}

/// Construct a thread ID in the canonical `TH-{uuid8}` format.
pub fn new_thread_id() -> String {
    let u = uuid::Uuid::new_v4().to_string();
    // Take first 8 chars of the UUID (before first `-`)
    let short = u.split('-').next().unwrap_or(&u[..8]);
    format!("TH-{}", short)
}
