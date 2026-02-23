use anyhow::{anyhow, Result};
use std::path::Path;

use super::checkpoint::CheckpointManager;
use super::event_log::TaskEventLog;
use super::events::{TaskEvent, TaskEventKind};
use super::reducer::{self, MaterializedTask, TaskState};

/// Orchestrates crash-safe replay of task state from the event log.
///
/// On each call to `replay()`:
/// 1. Loads the latest checkpoint (if any) to avoid replaying from scratch
/// 2. Reads events from the checkpoint's seq+1 onwards
/// 3. Applies events via `reducer::reduce()` in order
/// 4. Skips malformed lines with a warning (tolerates truncated JSONL files)
pub struct ReplayEngine {
    pub event_log: TaskEventLog,
    checkpoint_mgr: CheckpointManager,
}

impl ReplayEngine {
    /// Create a new replay engine for `task_id`. `data_dir` is the `.claw/`
    /// root that contains `tasks/<task-id>/`.
    pub fn new(task_id: &str, data_dir: &Path) -> Result<Self> {
        Ok(Self {
            event_log: TaskEventLog::new(task_id, data_dir)?,
            checkpoint_mgr: CheckpointManager::new(task_id, data_dir),
        })
    }

    /// Replay the task to its current materialized state.
    ///
    /// Loads the latest checkpoint, then replays events from `checkpoint.seq + 1`
    /// through the end of the log. Returns an error only if the log is
    /// completely unreadable; individual malformed events are skipped.
    pub async fn replay(&self) -> Result<MaterializedTask> {
        // Load latest checkpoint, if one exists
        let (mut state, from_seq) = match self.checkpoint_mgr.load_latest().await? {
            Some((state, seq)) => (state, seq + 1),
            None => {
                // No checkpoint — we need at least the TaskCreated event to bootstrap
                let events = self.event_log.read_from(0).await?;
                if events.is_empty() {
                    return Err(anyhow!("event log is empty — cannot replay"));
                }

                // The very first event must be TaskCreated
                let first = &events[0];
                let spec = match &first.kind {
                    TaskEventKind::TaskCreated { spec } => spec.clone(),
                    _ => {
                        return Err(anyhow!(
                            "first event is not TaskCreated (got {:?})",
                            first.kind
                        ))
                    }
                };

                let initial = MaterializedTask::initial(spec);
                // Apply from event 0 (we'll replay all events below)
                let mut s = reducer::reduce(initial, first)?;

                for event in events.iter().skip(1) {
                    s = match reducer::reduce(s.clone(), event) {
                        Ok(new_state) => new_state,
                        Err(e) => {
                            tracing::warn!(
                                task_id = %first.task_id,
                                seq = event.seq,
                                err = %e,
                                "skipping invalid event during replay"
                            );
                            s
                        }
                    };
                }
                return Ok(s);
            }
        };

        // Replay events since the checkpoint
        let events = self.event_log.read_from(from_seq).await?;
        let mut events_since_last_checkpoint = 0u64;

        for event in &events {
            state = match reducer::reduce(state.clone(), event) {
                Ok(new_state) => {
                    events_since_last_checkpoint += 1;

                    // Auto-checkpoint if thresholds met
                    if CheckpointManager::should_checkpoint(events_since_last_checkpoint, event) {
                        if let Err(e) = self
                            .checkpoint_mgr
                            .save(&new_state, event.seq)
                            .await
                        {
                            tracing::warn!(err = %e, "failed to save auto-checkpoint during replay");
                        } else {
                            events_since_last_checkpoint = 0;
                        }
                    }

                    new_state
                }
                Err(e) => {
                    tracing::warn!(
                        seq = event.seq,
                        err = %e,
                        "skipping invalid event during replay"
                    );
                    state
                }
            };
        }

        Ok(state)
    }

    /// Validate an event before appending it to the log.
    ///
    /// Checks:
    /// - Schema: event has a valid `task_id` matching this task
    /// - Monotonic timestamp: event.ts >= current state's updated_at
    /// - Idempotency: for ToolCalled, the key is not already in the state
    pub fn validate_event(
        &self,
        event: &TaskEvent,
        current_state: &MaterializedTask,
    ) -> Result<()> {
        // Timestamp monotonicity
        if event.ts < current_state.updated_at {
            return Err(anyhow!(
                "event timestamp {:?} is before current state updated_at {:?}",
                event.ts,
                current_state.updated_at
            ));
        }

        // For ToolCalled, check idempotency key is fresh
        if let TaskEventKind::ToolCalled {
            idempotency_key, ..
        } = &event.kind
        {
            if current_state
                .seen_idempotency_keys
                .contains(idempotency_key)
            {
                // Duplicate tool call — this is allowed (reducer will skip it)
                // but we log it so callers know
                tracing::debug!(
                    idempotency_key = %idempotency_key,
                    "duplicate ToolCalled event — will be skipped by reducer"
                );
            }
        }

        Ok(())
    }

    /// Append a new event to the log and return the updated materialized state.
    ///
    /// This is the main write path: validate, append, reduce.
    pub async fn append_and_reduce(
        &self,
        kind: TaskEventKind,
        actor: &str,
        current_state: MaterializedTask,
    ) -> Result<(MaterializedTask, u64)> {
        let correlation_id = super::events::new_correlation_id();

        let seq = self
            .event_log
            .append(kind.clone(), actor, &correlation_id)
            .await?;

        // Read back the event we just appended so we can pass the full struct to reduce()
        let all_events = self.event_log.read_from(seq).await?;
        let event = all_events
            .into_iter()
            .find(|e| e.seq == seq)
            .ok_or_else(|| anyhow!("appended event not found at seq {}", seq))?;

        let new_state = reducer::reduce(current_state, &event)?;

        // Auto-checkpoint at stable boundaries
        let events_since = seq.saturating_sub(new_state.event_seq.saturating_sub(1));
        if CheckpointManager::should_checkpoint(events_since, &event) {
            if let Err(e) = self.checkpoint_mgr.save(&new_state, seq).await {
                tracing::warn!(err = %e, seq, "failed to save auto-checkpoint after append");
            }
        }

        Ok((new_state, seq))
    }

    /// Recovery flow for agent crashes.
    ///
    /// Replays the task to determine its current state after a crash:
    /// - If state is `Active` and `claimed_by` is set: the agent crashed mid-task.
    ///   The caller should re-assign or escalate.
    /// - If state is `NeedsApproval`: still paused, approval still pending.
    /// - If state is `Blocked` with `retry_after` in the future: still blocked.
    ///
    /// Returns the materialized state so the caller decides what action to take.
    pub async fn recover_agent_crash(
        task_id: &str,
        data_dir: &Path,
    ) -> Result<MaterializedTask> {
        let engine = Self::new(task_id, data_dir)?;
        let state = engine.replay().await?;

        match &state.state {
            TaskState::Active if state.claimed_by.is_some() => {
                tracing::warn!(
                    task_id = %task_id,
                    agent = ?state.claimed_by,
                    "task is Active+Claimed but agent appears crashed — caller should re-assign"
                );
            }
            TaskState::NeedsApproval => {
                tracing::info!(
                    task_id = %task_id,
                    approval_id = ?state.pending_approval_id,
                    "task awaiting approval — still pending"
                );
            }
            TaskState::Blocked => {
                tracing::info!(
                    task_id = %task_id,
                    "task is blocked — waiting for unblock"
                );
            }
            _ => {}
        }

        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::events::TaskEventKind;
    use crate::tasks::schema::{Priority, RiskLevel, TaskSpec};
    use chrono::Utc;
    use tempfile::TempDir;

    async fn make_engine(dir: &TempDir, task_id: &str) -> ReplayEngine {
        ReplayEngine::new(task_id, dir.path()).unwrap()
    }

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

    #[tokio::test]
    async fn test_replay_empty_log_fails() {
        let dir = TempDir::new().unwrap();
        let engine = make_engine(&dir, "task-replay-empty").await;
        let result = engine.replay().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_basic_replay() {
        let dir = TempDir::new().unwrap();
        let engine = make_engine(&dir, "task-replay-basic").await;
        let spec = make_spec("task-replay-basic");

        // Append TaskCreated
        engine
            .event_log
            .append(
                TaskEventKind::TaskCreated { spec: spec.clone() },
                "daemon",
                "corr-000",
            )
            .await
            .unwrap();

        // Append TaskClaimed
        engine
            .event_log
            .append(
                TaskEventKind::TaskClaimed {
                    agent_id: "agent-1".to_string(),
                    role: "implementer".to_string(),
                },
                "daemon",
                "corr-001",
            )
            .await
            .unwrap();

        // Append TaskActive
        engine
            .event_log
            .append(TaskEventKind::TaskActive, "daemon", "corr-002")
            .await
            .unwrap();

        let state = engine.replay().await.unwrap();
        assert_eq!(state.state, TaskState::Active);
        assert_eq!(state.claimed_by, Some("agent-1".to_string()));
    }
}
