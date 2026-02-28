use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

use super::events::{TaskEvent, TaskEventKind};

/// Append-only JSONL event log for a single task.
///
/// Events are stored in `.claw/tasks/<task-id>/events.jsonl` — one JSON line
/// per event. The file grows monotonically; events are never deleted or
/// rewritten. Truncated last lines (from crashes) are handled gracefully on
/// read.
pub struct TaskEventLog {
    task_id: String,
    /// Path to the `.claw/tasks/<task-id>/` directory.
    log_dir: PathBuf,
}

impl TaskEventLog {
    /// Create a new event log handle. `data_dir` is the `.claw/` root
    /// (or any dir where `tasks/<task-id>/` lives).
    pub fn new(task_id: &str, data_dir: &Path) -> Result<Self> {
        let log_dir = data_dir.join("tasks").join(task_id);
        Ok(Self {
            task_id: task_id.to_string(),
            log_dir,
        })
    }

    fn log_path(&self) -> PathBuf {
        self.log_dir.join("events.jsonl")
    }

    /// Ensure the task directory exists.
    async fn ensure_dir(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.log_dir).await?;
        Ok(())
    }

    /// Append a new event. Returns the assigned sequence number.
    ///
    /// The sequence number equals the count of events already in the log
    /// at the time of append (0-indexed). This function opens the file in
    /// append mode, so concurrent appends from multiple tasks are safe at
    /// the file-system level (different files per task).
    pub async fn append(
        &self,
        kind: TaskEventKind,
        actor: &str,
        correlation_id: &str,
    ) -> Result<u64> {
        self.ensure_dir().await?;

        let seq = self.event_count().await?;
        let event = TaskEvent::new(&self.task_id, seq, actor, correlation_id, kind);
        let mut line = serde_json::to_string(&event)?;
        line.push('\n');

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())
            .await?;

        file.write_all(line.as_bytes()).await?;
        file.flush().await?;

        Ok(seq)
    }

    /// Read all events, optionally starting after `from_seq` (exclusive).
    ///
    /// Handles truncated last lines gracefully: malformed JSON is skipped
    /// with a warning. The caller receives only valid events.
    pub async fn read_from(&self, from_seq: u64) -> Result<Vec<TaskEvent>> {
        let path = self.log_path();
        if !path.exists() {
            return Ok(vec![]);
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let mut events = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            match serde_json::from_str::<TaskEvent>(line) {
                Ok(event) => {
                    if event.seq >= from_seq {
                        events.push(event);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        task_id = %self.task_id,
                        line = line_num + 1,
                        err = %e,
                        "skipping malformed event log line"
                    );
                }
            }
        }

        Ok(events)
    }

    /// Current event count (next seq = count).
    ///
    /// Counts non-empty, valid lines in the JSONL file. Malformed lines are
    /// skipped and do not increment the count, preserving monotonic seq numbers.
    pub async fn event_count(&self) -> Result<u64> {
        let path = self.log_path();
        if !path.exists() {
            return Ok(0);
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let mut count = 0u64;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // Only count valid JSON lines — malformed lines from crashes are skipped
            if serde_json::from_str::<serde_json::Value>(line).is_ok() {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Delete the entire event log directory for this task.
    /// Used only in tests or explicit cleanup — not called in normal operation.
    #[cfg(test)]
    pub async fn delete(&self) -> Result<()> {
        if self.log_dir.exists() {
            tokio::fs::remove_dir_all(&self.log_dir).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn make_log(dir: &TempDir) -> TaskEventLog {
        TaskEventLog::new("test-task-001", dir.path()).unwrap()
    }

    #[tokio::test]
    async fn test_empty_log() {
        let dir = TempDir::new().unwrap();
        let log = make_log(&dir).await;
        assert_eq!(log.event_count().await.unwrap(), 0);
        assert!(log.read_from(0).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_append_and_read() {
        use crate::tasks::schema::{Priority, RiskLevel, TaskSpec};
        use chrono::Utc;

        let dir = TempDir::new().unwrap();
        let log = make_log(&dir).await;

        let spec = TaskSpec {
            id: "test-task-001".to_string(),
            title: "Test task".to_string(),
            repo: "/tmp/test".to_string(),
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

        let seq0 = log
            .append(TaskEventKind::TaskCreated { spec }, "daemon", "corr-000")
            .await
            .unwrap();
        assert_eq!(seq0, 0);

        let seq1 = log
            .append(TaskEventKind::TaskActive, "daemon", "corr-001")
            .await
            .unwrap();
        assert_eq!(seq1, 1);

        let all = log.read_from(0).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].seq, 0);
        assert_eq!(all[1].seq, 1);

        let from_1 = log.read_from(1).await.unwrap();
        assert_eq!(from_1.len(), 1);
        assert_eq!(from_1[0].seq, 1);
    }
}
