use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::events::{TaskEvent, TaskEventKind};
use super::schema::TaskSpec;

/// The finite set of states a task can be in.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    Planned,
    Claimed,
    Active,
    Blocked,
    NeedsApproval,
    CodeReview,
    Qa,
    Done,
    Canceled,
    Failed,
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| format!("{:?}", self));
        write!(f, "{}", s)
    }
}

/// Fully materialized task state, built by replaying events through the reducer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterializedTask {
    pub spec: TaskSpec,
    pub state: TaskState,
    pub claimed_by: Option<String>,
    pub pending_approval_id: Option<String>,
    /// Sequence number of the last event that was applied.
    pub event_seq: u64,
    /// Idempotency keys seen for ToolCalled events. Prevents duplicate processing.
    pub seen_idempotency_keys: HashSet<String>,
    pub updated_at: DateTime<Utc>,
}

impl MaterializedTask {
    /// Create initial materialized state from a TaskSpec.
    pub fn initial(spec: TaskSpec) -> Self {
        Self {
            updated_at: spec.created_at,
            spec,
            state: TaskState::Pending,
            claimed_by: None,
            pending_approval_id: None,
            event_seq: 0,
            seen_idempotency_keys: HashSet::new(),
        }
    }
}

/// Pure function: apply one event to the current materialized state and return
/// the new state. Returns `Err` if the transition is invalid.
///
/// This function is the heart of the Task State Engine. It is deterministic —
/// given the same state and event it always produces the same result — which
/// makes the full replay algorithm reliable.
pub fn reduce(mut state: MaterializedTask, event: &TaskEvent) -> Result<MaterializedTask> {
    state.event_seq = event.seq;
    state.updated_at = event.ts;

    match &event.kind {
        // ── Task created ─────────────────────────────────────────────────────
        TaskEventKind::TaskCreated { spec } => {
            // Only valid as the very first event (seq 0 from Pending).
            // We replace the spec in case this is applied during initial build.
            state.spec = spec.clone();
            state.state = TaskState::Pending;
        }

        // ── Task planned ─────────────────────────────────────────────────────
        TaskEventKind::TaskPlanned { .. } => match state.state {
            TaskState::Pending => {
                state.state = TaskState::Planned;
            }
            _ => {
                return Err(anyhow!(
                    "invalid transition: TaskPlanned from {:?}",
                    state.state
                ))
            }
        },

        // ── Task claimed ─────────────────────────────────────────────────────
        // Claimed is a sub-state: it sets claimed_by but does not change the
        // primary state (Pending or Planned remain — Active requires TaskActive).
        TaskEventKind::TaskClaimed { agent_id, .. } => match state.state {
            TaskState::Pending | TaskState::Planned => {
                state.claimed_by = Some(agent_id.clone());
            }
            _ => {
                return Err(anyhow!(
                    "invalid transition: TaskClaimed from {:?}",
                    state.state
                ))
            }
        },

        // ── Task active ──────────────────────────────────────────────────────
        TaskEventKind::TaskActive => match state.state {
            TaskState::Pending
            | TaskState::Planned
            | TaskState::Blocked
            | TaskState::CodeReview
            | TaskState::Qa => {
                state.state = TaskState::Active;
                state.pending_approval_id = None;
            }
            // NeedsApproval is intentionally excluded here.
            // The only valid path from NeedsApproval → Active is via ApprovalGranted,
            // which is handled below. Allowing TaskActive from NeedsApproval would
            // let agents self-approve and bypass the human approval gate.
            _ => {
                return Err(anyhow!(
                    "invalid transition: TaskActive from {:?}",
                    state.state
                ))
            }
        },

        // ── Task blocked ─────────────────────────────────────────────────────
        TaskEventKind::TaskBlocked { .. } => match state.state {
            TaskState::Active | TaskState::Blocked => {
                state.state = TaskState::Blocked;
            }
            _ => {
                return Err(anyhow!(
                    "invalid transition: TaskBlocked from {:?}",
                    state.state
                ))
            }
        },

        // ── Needs approval ───────────────────────────────────────────────────
        TaskEventKind::TaskNeedsApproval { approval_id, .. } => match state.state {
            TaskState::Active => {
                state.state = TaskState::NeedsApproval;
                state.pending_approval_id = Some(approval_id.clone());
            }
            _ => {
                return Err(anyhow!(
                    "invalid transition: TaskNeedsApproval from {:?}",
                    state.state
                ))
            }
        },

        // ── Code review ──────────────────────────────────────────────────────
        TaskEventKind::TaskCodeReview { .. } => match state.state {
            TaskState::Active => {
                state.state = TaskState::CodeReview;
            }
            _ => {
                return Err(anyhow!(
                    "invalid transition: TaskCodeReview from {:?}",
                    state.state
                ))
            }
        },

        // ── QA ───────────────────────────────────────────────────────────────
        TaskEventKind::TaskQa { .. } => match state.state {
            TaskState::CodeReview => {
                state.state = TaskState::Qa;
            }
            _ => {
                return Err(anyhow!(
                    "invalid transition: TaskQa from {:?}",
                    state.state
                ))
            }
        },

        // ── Done ─────────────────────────────────────────────────────────────
        TaskEventKind::TaskDone { .. } => match state.state {
            TaskState::Qa | TaskState::Active => {
                state.state = TaskState::Done;
            }
            _ => {
                return Err(anyhow!(
                    "invalid transition: TaskDone from {:?}",
                    state.state
                ))
            }
        },

        // ── Canceled — from any non-terminal state ────────────────────────────
        TaskEventKind::TaskCanceled { .. } => match state.state {
            TaskState::Done | TaskState::Canceled | TaskState::Failed => {
                return Err(anyhow!(
                    "invalid transition: TaskCanceled from terminal state {:?}",
                    state.state
                ))
            }
            _ => {
                state.state = TaskState::Canceled;
            }
        },

        // ── Failed — from any non-terminal state ──────────────────────────────
        TaskEventKind::TaskFailed { .. } => match state.state {
            TaskState::Done | TaskState::Canceled | TaskState::Failed => {
                return Err(anyhow!(
                    "invalid transition: TaskFailed from terminal state {:?}",
                    state.state
                ))
            }
            _ => {
                state.state = TaskState::Failed;
            }
        },

        // ── Tool called (idempotent) ──────────────────────────────────────────
        TaskEventKind::ToolCalled {
            idempotency_key, ..
        } => {
            if state.seen_idempotency_keys.contains(idempotency_key) {
                // Duplicate — skip silently. State unchanged.
                return Ok(state);
            }
            state.seen_idempotency_keys.insert(idempotency_key.clone());
            // State itself does not change; tool calls happen within Active.
        }

        // ── Tool result ───────────────────────────────────────────────────────
        TaskEventKind::ToolResult {
            idempotency_key, ..
        } => {
            // Track result — idempotency key should already be in the set.
            // We accept it even if not (log was partially truncated).
            state.seen_idempotency_keys.insert(idempotency_key.clone());
        }

        // ── Checkpoint ───────────────────────────────────────────────────────
        TaskEventKind::CheckpointCreated { .. } => {
            // No state change — just a marker in the log.
        }

        // ── Approval requested ───────────────────────────────────────────────
        TaskEventKind::ApprovalRequested { approval_id, .. } => {
            state.pending_approval_id = Some(approval_id.clone());
        }

        // ── Approval granted ─────────────────────────────────────────────────
        TaskEventKind::ApprovalGranted { .. } => {
            state.pending_approval_id = None;
            // Transition NeedsApproval → Active
            if state.state == TaskState::NeedsApproval {
                state.state = TaskState::Active;
            }
        }

        // ── Approval denied ──────────────────────────────────────────────────
        TaskEventKind::ApprovalDenied { .. } => {
            state.pending_approval_id = None;
            // Transition NeedsApproval → Blocked
            if state.state == TaskState::NeedsApproval {
                state.state = TaskState::Blocked;
            }
        }
    }

    Ok(state)
}

/// Check whether file edits are allowed in the given task state.
///
/// Edits are only permitted when the task is Active AND claimed by an agent.
/// Returns `Ok(())` if allowed, `Err` otherwise.
pub fn check_write_allowed(state: &MaterializedTask) -> Result<()> {
    if state.state == TaskState::Active && state.claimed_by.is_some() {
        Ok(())
    } else {
        Err(anyhow!(
            "task must be Active+Claimed to edit files (current state: {}, claimed_by: {:?})",
            state.state,
            state.claimed_by
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::schema::{Priority, RiskLevel, TaskSpec};
    use chrono::Utc;

    fn make_spec(id: &str) -> TaskSpec {
        TaskSpec {
            id: id.to_string(),
            title: "Test".to_string(),
            repo: "/tmp".to_string(),
            summary: None,
            acceptance_criteria: vec![],
            test_plan: None,
            risk_level: RiskLevel::Low,
            priority: Priority::Medium,
            labels: vec![],
            owner: None,
            worktree_path: None,
            worktree_branch: None,
            created_at: Utc::now(),
        }
    }

    fn make_event(task_id: &str, seq: u64, kind: TaskEventKind) -> TaskEvent {
        TaskEvent::new(task_id, seq, "daemon", "corr-test", kind)
    }

    #[test]
    fn test_pending_to_active() {
        let spec = make_spec("t1");
        let state = MaterializedTask::initial(spec.clone());
        let event = make_event("t1", 1, TaskEventKind::TaskActive);
        let new_state = reduce(state, &event).unwrap();
        assert_eq!(new_state.state, TaskState::Active);
    }

    #[test]
    fn test_check_write_allowed_active_claimed() {
        let spec = make_spec("t1");
        let mut state = MaterializedTask::initial(spec);
        state.state = TaskState::Active;
        state.claimed_by = Some("agent-1".to_string());
        assert!(check_write_allowed(&state).is_ok());
    }

    #[test]
    fn test_check_write_allowed_pending() {
        let spec = make_spec("t1");
        let state = MaterializedTask::initial(spec);
        assert!(check_write_allowed(&state).is_err());
    }

    #[test]
    fn test_tool_call_idempotency() {
        let spec = make_spec("t1");
        let mut state = MaterializedTask::initial(spec);
        state.state = TaskState::Active;

        let e1 = make_event(
            "t1",
            1,
            TaskEventKind::ToolCalled {
                tool_name: "read_file".to_string(),
                arguments_hash: "abc".to_string(),
                idempotency_key: "key-001".to_string(),
            },
        );
        let state = reduce(state, &e1).unwrap();
        assert!(state.seen_idempotency_keys.contains("key-001"));

        // Apply the same event again — should be skipped
        let e2 = make_event(
            "t1",
            2,
            TaskEventKind::ToolCalled {
                tool_name: "read_file".to_string(),
                arguments_hash: "abc".to_string(),
                idempotency_key: "key-001".to_string(),
            },
        );
        let state2 = reduce(state.clone(), &e2).unwrap();
        assert_eq!(state2.state, state.state);
    }

    #[test]
    fn test_invalid_transition() {
        let spec = make_spec("t1");
        let state = MaterializedTask::initial(spec);
        // Cannot go Done from Pending
        let event = make_event(
            "t1",
            1,
            TaskEventKind::TaskDone {
                completion_notes: "done".to_string(),
            },
        );
        assert!(reduce(state, &event).is_err());
    }
}
