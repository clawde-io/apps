//! Task Engine data model types.

use serde::{Deserialize, Serialize};

/// Generate a new ULID string.
pub fn new_id() -> String {
    ulid::Ulid::new().to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TePhase {
    pub id: String,
    pub display_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub planning_doc_path: Option<String>,
    pub repo: Option<String>,
    pub priority: String,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TeTask {
    pub id: String,
    pub display_id: String,
    pub phase_id: String,
    pub parent_task_id: Option<String>,
    pub depth: i64,
    pub title: String,
    pub description: String,
    pub ai_instructions: String,
    pub requirements: String,       // JSON array
    pub definition_of_done: String, // JSON array
    pub cr_checklist: Option<String>,
    pub qa_checklist: Option<String>,
    pub task_type: String,
    pub priority: String,
    pub risk_level: String,
    pub status: String,
    pub blocked_reason: Option<String>,
    pub pause_reason: Option<String>,
    pub failure_reason: Option<String>,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<i64>,
    pub reviewer_agent_id: Option<String>,
    pub qa_agent_id: Option<String>,
    pub estimated_tokens: Option<i64>,
    pub estimated_minutes: Option<i64>,
    pub estimated_files: Option<i64>,
    pub repo: Option<String>,
    pub target_files: Option<String>,
    pub worktree_path: Option<String>,
    pub branch_name: Option<String>,
    pub retry_count: i64,
    pub max_retries: i64,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub discovered_from_task_id: Option<String>,
    pub tags: Option<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TeAgent {
    pub id: String,
    pub name: String,
    pub agent_type: String,
    pub role: String,
    pub session_id: Option<String>,
    pub connection_type: String,
    pub status: String,
    pub current_task_id: Option<String>,
    pub last_heartbeat_at: i64,
    pub heartbeat_interval_secs: i64,
    pub heartbeat_timeout_secs: i64,
    pub capabilities: String, // JSON array
    pub max_context_tokens: Option<i64>,
    pub model_id: Option<String>,
    pub tasks_completed: i64,
    pub tasks_failed: i64,
    pub total_tokens_used: i64,
    pub avg_task_duration_secs: Option<i64>,
    pub registered_at: i64,
    pub last_active_at: Option<i64>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TeEvent {
    pub id: String,
    pub task_id: String,
    pub agent_id: Option<String>,
    pub event_seq: i64,
    pub event_type: String,
    pub payload: String, // JSON
    pub idempotency_key: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TeNote {
    pub id: String,
    pub task_id: String,
    pub agent_id: Option<String>,
    pub note_type: String,
    pub title: String,
    pub content: String,
    pub related_file: Option<String>,
    pub related_line: Option<i64>,
    pub visibility: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TeCheckpoint {
    pub id: String,
    pub task_id: String,
    pub agent_id: String,
    pub checkpoint_type: String,
    pub completed_items: String, // JSON array
    pub files_modified: String,  // JSON array
    pub tests_run: Option<String>,
    pub builds_run: Option<String>,
    pub current_action: String,
    pub current_file: Option<String>,
    pub partial_work: Option<String>,
    pub next_steps: String,       // JSON array
    pub remaining_items: String,  // JSON array
    pub key_discoveries: Option<String>,
    pub decisions_made: Option<String>,
    pub gotchas: Option<String>,
    pub patterns_observed: Option<String>,
    pub context_summary: Option<String>,
    pub environment_state: Option<String>,
    pub last_event_seq: i64,
    pub timestamp: i64,
}

/// Valid task status transitions.
pub fn valid_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("planned", "ready")
            | ("planned", "canceled")
            | ("ready", "queued")
            | ("ready", "canceled")
            | ("queued", "claimed")
            | ("queued", "canceled")
            | ("claimed", "active")
            | ("claimed", "queued") // release
            | ("claimed", "canceled")
            | ("active", "paused")
            | ("active", "blocked")
            | ("active", "needs_review")
            | ("active", "canceled")
            | ("active", "failed")
            | ("paused", "active")
            | ("paused", "canceled")
            | ("blocked", "active")
            | ("blocked", "canceled")
            | ("needs_review", "in_review")
            | ("in_review", "needs_qa")
            | ("in_review", "review_failed")
            | ("review_failed", "active")
            | ("needs_qa", "in_qa")
            | ("in_qa", "needs_secondary")
            | ("in_qa", "qa_failed")
            | ("qa_failed", "active")
            | ("needs_secondary", "done")
            | (_, "canceled")
            | (_, "failed")
    )
}
