use anyhow::Result;
use std::path::{Path, PathBuf};

use super::events::{TaskEvent, TaskEventKind};
use super::reducer::MaterializedTask;

/// Manages periodic checkpoint snapshots for a task's materialized state.
///
/// Checkpoints are stored as JSON files named `CP-{seq:010}.json` in
/// `.claw/tasks/<task-id>/checkpoints/`. The zero-padded name ensures
/// lexicographic sort order matches event order.
pub struct CheckpointManager {
    task_id: String,
    /// Path to `.claw/tasks/<task-id>/checkpoints/`
    checkpoint_dir: PathBuf,
}

impl CheckpointManager {
    pub fn new(task_id: &str, data_dir: &Path) -> Self {
        Self {
            task_id: task_id.to_string(),
            checkpoint_dir: data_dir.join("tasks").join(task_id).join("checkpoints"),
        }
    }

    /// Save the current materialized state as a checkpoint at `event_seq`.
    /// Returns the path of the written checkpoint file.
    pub async fn save(&self, state: &MaterializedTask, event_seq: u64) -> Result<PathBuf> {
        tokio::fs::create_dir_all(&self.checkpoint_dir).await?;

        let filename = format!("CP-{:010}.json", event_seq);
        let path = self.checkpoint_dir.join(&filename);
        let json = serde_json::to_string_pretty(state)?;
        tokio::fs::write(&path, json).await?;

        tracing::debug!(
            task_id = %self.task_id,
            event_seq,
            file = %filename,
            "checkpoint saved"
        );

        Ok(path)
    }

    /// Load the latest checkpoint. Returns `(state, event_seq)` or `None` if
    /// no checkpoints exist yet.
    ///
    /// Iterates checkpoint files in reverse lexicographic order (latest first)
    /// and returns the first one that deserializes successfully.
    pub async fn load_latest(&self) -> Result<Option<(MaterializedTask, u64)>> {
        if !self.checkpoint_dir.exists() {
            return Ok(None);
        }

        let mut entries: Vec<PathBuf> = vec![];
        let mut dir = tokio::fs::read_dir(&self.checkpoint_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("CP-") && n.ends_with(".json"))
                .unwrap_or(false)
            {
                entries.push(path);
            }
        }

        // Sort descending so the latest checkpoint is first
        entries.sort_by(|a, b| b.cmp(a));

        for path in entries {
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => match serde_json::from_str::<MaterializedTask>(&content) {
                    Ok(state) => {
                        let seq = state.event_seq;
                        tracing::debug!(
                            task_id = %self.task_id,
                            event_seq = seq,
                            file = %path.display(),
                            "checkpoint loaded"
                        );
                        return Ok(Some((state, seq)));
                    }
                    Err(e) => {
                        tracing::warn!(
                            task_id = %self.task_id,
                            file = %path.display(),
                            err = %e,
                            "skipping corrupt checkpoint file"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        task_id = %self.task_id,
                        file = %path.display(),
                        err = %e,
                        "could not read checkpoint file"
                    );
                }
            }
        }

        Ok(None)
    }

    /// Returns true if a checkpoint should be created now.
    ///
    /// Checkpoints at:
    /// - Every 50 events since the last checkpoint
    /// - At stable state boundaries: CheckpointCreated, Done, CodeReview, Qa
    pub fn should_checkpoint(events_since_last: u64, event: &TaskEvent) -> bool {
        if events_since_last >= 50 {
            return true;
        }
        matches!(
            event.kind,
            TaskEventKind::CheckpointCreated { .. }
                | TaskEventKind::TaskDone { .. }
                | TaskEventKind::TaskCodeReview { .. }
                | TaskEventKind::TaskQa { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::reducer::TaskState;
    use crate::tasks::schema::{Priority, RiskLevel, TaskSpec};
    use chrono::Utc;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_state(task_id: &str) -> MaterializedTask {
        let spec = TaskSpec {
            id: task_id.to_string(),
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
        };
        MaterializedTask {
            spec,
            state: TaskState::Active,
            claimed_by: Some("agent-1".to_string()),
            pending_approval_id: None,
            event_seq: 10,
            seen_idempotency_keys: HashSet::new(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_save_and_load_checkpoint() {
        let dir = TempDir::new().unwrap();
        let mgr = CheckpointManager::new("task-cp-test", dir.path());
        let state = make_state("task-cp-test");

        let path = mgr.save(&state, 10).await.unwrap();
        assert!(path.exists());

        let loaded = mgr.load_latest().await.unwrap();
        assert!(loaded.is_some());
        let (loaded_state, seq) = loaded.unwrap();
        assert_eq!(seq, 10);
        assert_eq!(loaded_state.state, TaskState::Active);
    }

    #[tokio::test]
    async fn test_load_latest_when_empty() {
        let dir = TempDir::new().unwrap();
        let mgr = CheckpointManager::new("task-empty", dir.path());
        let result = mgr.load_latest().await.unwrap();
        assert!(result.is_none());
    }
}
