use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::schema::TaskSpec;

/// Generate a new correlation ID (UUID v4).
pub fn new_correlation_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// All distinct event kinds the Task State Engine can record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum TaskEventKind {
    // ── Task lifecycle ────────────────────────────────────────────────────────
    TaskCreated {
        spec: TaskSpec,
    },
    TaskPlanned {
        phases: Vec<Value>,
    },
    TaskClaimed {
        agent_id: String,
        role: String,
    },
    TaskActive,
    TaskBlocked {
        reason: String,
        retry_after: Option<DateTime<Utc>>,
    },
    TaskNeedsApproval {
        approval_id: String,
        tool_name: String,
        risk_level: String,
    },
    TaskCodeReview {
        reviewer_id: Option<String>,
    },
    TaskQa {
        qa_agent_id: Option<String>,
    },
    TaskDone {
        completion_notes: String,
    },
    TaskCanceled {
        reason: String,
    },
    TaskFailed {
        error: String,
    },
    // ── Tool events ───────────────────────────────────────────────────────────
    ToolCalled {
        tool_name: String,
        arguments_hash: String,
        idempotency_key: String,
    },
    ToolResult {
        idempotency_key: String,
        success: bool,
        output_summary: String,
    },
    // ── Checkpoint ────────────────────────────────────────────────────────────
    CheckpointCreated {
        seq: u64,
    },
    // ── Approval ──────────────────────────────────────────────────────────────
    ApprovalRequested {
        approval_id: String,
        tool_name: String,
        risk_level: String,
    },
    ApprovalGranted {
        approval_id: String,
        granted_by: String,
    },
    ApprovalDenied {
        approval_id: String,
        denied_by: String,
        reason: String,
    },
}

/// A single immutable event in the task event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEvent {
    pub task_id: String,
    pub seq: u64,
    pub ts: DateTime<Utc>,
    /// Actor that caused this event: agent_id, "user", or "daemon".
    pub actor: String,
    /// UUID for request tracing across distributed logs.
    pub correlation_id: String,
    #[serde(flatten)]
    pub kind: TaskEventKind,
}

impl TaskEvent {
    /// Create a new event. The seq is assigned by the event log on append.
    pub fn new(
        task_id: &str,
        seq: u64,
        actor: &str,
        correlation_id: &str,
        kind: TaskEventKind,
    ) -> Self {
        Self {
            task_id: task_id.to_string(),
            seq,
            ts: Utc::now(),
            actor: actor.to_string(),
            correlation_id: correlation_id.to_string(),
            kind,
        }
    }
}
