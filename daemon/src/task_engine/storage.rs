//! Task Engine SQLite operations.

use anyhow::Result;
use sqlx::SqlitePool;

use super::model::*;

pub struct TaskEngineStorage {
    pub(crate) pool: SqlitePool,
}

impl TaskEngineStorage {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ─── Phases ───────────────────────────────────────────────────────────

    pub async fn create_phase(
        &self,
        display_id: &str,
        title: &str,
        description: &str,
        priority: &str,
        planning_doc_path: Option<&str>,
        repo: Option<&str>,
    ) -> Result<TePhase> {
        let id = new_id();
        sqlx::query(
            "INSERT INTO te_phases (id, display_id, title, description, priority, planning_doc_path, repo) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(display_id)
        .bind(title)
        .bind(description)
        .bind(priority)
        .bind(planning_doc_path)
        .bind(repo)
        .execute(&self.pool)
        .await?;
        self.get_phase(&id).await
    }

    pub async fn get_phase(&self, id: &str) -> Result<TePhase> {
        Ok(sqlx::query_as("SELECT * FROM te_phases WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn list_phases(&self) -> Result<Vec<TePhase>> {
        Ok(
            sqlx::query_as("SELECT * FROM te_phases ORDER BY created_at ASC")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn update_phase_status(&self, id: &str, status: &str) -> Result<()> {
        let now = unixepoch();
        match status {
            "active" => {
                sqlx::query("UPDATE te_phases SET status = ?, started_at = ? WHERE id = ?")
                    .bind(status)
                    .bind(now)
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
            "completed" => {
                sqlx::query("UPDATE te_phases SET status = ?, completed_at = ? WHERE id = ?")
                    .bind(status)
                    .bind(now)
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
            _ => {
                sqlx::query("UPDATE te_phases SET status = ? WHERE id = ?")
                    .bind(status)
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }

    // ─── Tasks ────────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub async fn create_task(
        &self,
        display_id: &str,
        phase_id: &str,
        parent_task_id: Option<&str>,
        title: &str,
        description: &str,
        task_type: &str,
        priority: &str,
    ) -> Result<TeTask> {
        let id = new_id();
        let depth: i64 = if parent_task_id.is_some() { 1 } else { 0 };
        sqlx::query(
            "INSERT INTO te_tasks \
             (id, display_id, phase_id, parent_task_id, depth, title, description, task_type, priority) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(display_id)
        .bind(phase_id)
        .bind(parent_task_id)
        .bind(depth)
        .bind(title)
        .bind(description)
        .bind(task_type)
        .bind(priority)
        .execute(&self.pool)
        .await?;
        self.get_task(&id).await
    }

    pub async fn get_task(&self, id: &str) -> Result<TeTask> {
        Ok(sqlx::query_as("SELECT * FROM te_tasks WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn list_tasks(
        &self,
        phase_id: Option<&str>,
        status: Option<&str>,
    ) -> Result<Vec<TeTask>> {
        match (phase_id, status) {
            (Some(p), Some(s)) => Ok(sqlx::query_as(
                "SELECT * FROM te_tasks WHERE phase_id = ? AND status = ? ORDER BY created_at ASC",
            )
            .bind(p)
            .bind(s)
            .fetch_all(&self.pool)
            .await?),
            (Some(p), None) => Ok(sqlx::query_as(
                "SELECT * FROM te_tasks WHERE phase_id = ? ORDER BY created_at ASC",
            )
            .bind(p)
            .fetch_all(&self.pool)
            .await?),
            (None, Some(s)) => Ok(sqlx::query_as(
                "SELECT * FROM te_tasks WHERE status = ? ORDER BY priority DESC, created_at ASC",
            )
            .bind(s)
            .fetch_all(&self.pool)
            .await?),
            (None, None) => Ok(
                sqlx::query_as("SELECT * FROM te_tasks ORDER BY created_at ASC")
                    .fetch_all(&self.pool)
                    .await?,
            ),
        }
    }

    /// Atomically claim a task. Returns the task if successfully claimed, None if already claimed.
    pub async fn claim_task(&self, task_id: &str, agent_id: &str) -> Result<Option<TeTask>> {
        let now = unixepoch();
        let rows_affected = sqlx::query(
            "UPDATE te_tasks SET status = 'claimed', claimed_by = ?, claimed_at = ? \
             WHERE id = ? AND status IN ('queued', 'ready') AND claimed_by IS NULL",
        )
        .bind(agent_id)
        .bind(now)
        .bind(task_id)
        .execute(&self.pool)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Ok(None);
        }

        // Update agent's current task
        sqlx::query(
            "UPDATE te_agents SET status = 'working', current_task_id = ?, last_active_at = ? WHERE id = ?",
        )
        .bind(task_id)
        .bind(now)
        .bind(agent_id)
        .execute(&self.pool)
        .await?;

        Ok(Some(self.get_task(task_id).await?))
    }

    /// Claim the next available task for a given role/priority.
    pub async fn claim_next_task(
        &self,
        agent_id: &str,
        _role: &str,
        _priority_filter: Option<&[&str]>,
    ) -> Result<Option<TeTask>> {
        let candidates: Vec<TeTask> = sqlx::query_as(
            "SELECT * FROM te_tasks \
             WHERE status IN ('queued','ready') AND claimed_by IS NULL \
             ORDER BY \
               CASE priority WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END, \
               created_at ASC \
             LIMIT 10",
        )
        .fetch_all(&self.pool)
        .await?;

        for task in candidates {
            if let Some(claimed) = self.claim_task(&task.id, agent_id).await? {
                return Ok(Some(claimed));
            }
        }
        Ok(None)
    }

    pub async fn transition_task(
        &self,
        task_id: &str,
        new_status: &str,
        reason: Option<&str>,
    ) -> Result<TeTask> {
        let now = unixepoch();
        let task = self.get_task(task_id).await?;

        if !valid_transition(&task.status, new_status) {
            anyhow::bail!("invalid transition: {} -> {}", task.status, new_status);
        }

        let block_r: Option<&str> = if new_status == "blocked" {
            reason
        } else {
            None
        };
        let pause_r: Option<&str> = if new_status == "paused" { reason } else { None };
        let fail_r: Option<&str> = if new_status == "failed" { reason } else { None };

        // Use separate queries to avoid dynamic SQL complexity
        if new_status == "active" && task.started_at.is_none() {
            sqlx::query(
                "UPDATE te_tasks SET status = ?, blocked_reason = ?, pause_reason = ?, \
                 failure_reason = ?, started_at = ? WHERE id = ?",
            )
            .bind(new_status)
            .bind(block_r)
            .bind(pause_r)
            .bind(fail_r)
            .bind(now)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        } else if new_status == "done" {
            sqlx::query(
                "UPDATE te_tasks SET status = ?, blocked_reason = ?, pause_reason = ?, \
                 failure_reason = ?, completed_at = ? WHERE id = ?",
            )
            .bind(new_status)
            .bind(block_r)
            .bind(pause_r)
            .bind(fail_r)
            .bind(now)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "UPDATE te_tasks SET status = ?, blocked_reason = ?, pause_reason = ?, \
                 failure_reason = ? WHERE id = ?",
            )
            .bind(new_status)
            .bind(block_r)
            .bind(pause_r)
            .bind(fail_r)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        }

        // Log the transition event
        self.append_event(
            task_id,
            None,
            &format!("task.{}", task_status_to_event(new_status)),
            "{}",
            None,
        )
        .await?;

        self.get_task(task_id).await
    }

    // ─── Agents ───────────────────────────────────────────────────────────

    pub async fn register_agent(
        &self,
        name: &str,
        agent_type: &str,
        role: &str,
        capabilities: &str,
        model_id: Option<&str>,
        max_context_tokens: Option<i64>,
    ) -> Result<TeAgent> {
        let id = new_id();
        sqlx::query(
            "INSERT INTO te_agents \
             (id, name, agent_type, role, capabilities, model_id, max_context_tokens) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(agent_type)
        .bind(role)
        .bind(capabilities)
        .bind(model_id)
        .bind(max_context_tokens)
        .execute(&self.pool)
        .await?;
        self.get_agent(&id).await
    }

    pub async fn get_agent(&self, id: &str) -> Result<TeAgent> {
        Ok(sqlx::query_as("SELECT * FROM te_agents WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn heartbeat(
        &self,
        agent_id: &str,
        status: &str,
        current_task_id: Option<&str>,
    ) -> Result<()> {
        let now = unixepoch();
        sqlx::query(
            "UPDATE te_agents \
             SET last_heartbeat_at = ?, last_active_at = ?, status = ?, current_task_id = ? \
             WHERE id = ?",
        )
        .bind(now)
        .bind(now)
        .bind(status)
        .bind(current_task_id)
        .bind(agent_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn deregister_agent(&self, agent_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE te_agents SET status = 'terminated', current_task_id = NULL WHERE id = ?",
        )
        .bind(agent_id)
        .execute(&self.pool)
        .await?;
        // Release any claimed tasks
        sqlx::query(
            "UPDATE te_tasks SET status = 'queued', claimed_by = NULL, claimed_at = NULL \
             WHERE claimed_by = ? AND status IN ('claimed','active','paused')",
        )
        .bind(agent_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn detect_dead_agents(&self) -> Result<Vec<String>> {
        let dead: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM te_agents WHERE status = 'working' \
             AND last_heartbeat_at < (unixepoch() - heartbeat_timeout_secs)",
        )
        .fetch_all(&self.pool)
        .await?;
        let ids: Vec<String> = dead.into_iter().map(|(id,)| id).collect();
        for id in &ids {
            self.deregister_agent(id).await?;
        }
        Ok(ids)
    }

    // ─── Events ───────────────────────────────────────────────────────────

    pub async fn append_event(
        &self,
        task_id: &str,
        agent_id: Option<&str>,
        event_type: &str,
        payload: &str,
        idempotency_key: Option<&str>,
    ) -> Result<TeEvent> {
        let id = new_id();
        let (seq,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(event_seq), -1) + 1 FROM te_events WHERE task_id = ?",
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await?;

        sqlx::query(
            "INSERT OR IGNORE INTO te_events \
             (id, task_id, agent_id, event_seq, event_type, payload, idempotency_key) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(task_id)
        .bind(agent_id)
        .bind(seq)
        .bind(event_type)
        .bind(payload)
        .bind(idempotency_key)
        .execute(&self.pool)
        .await?;

        Ok(sqlx::query_as("SELECT * FROM te_events WHERE id = ?")
            .bind(&id)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn list_events(&self, task_id: &str, limit: i64) -> Result<Vec<TeEvent>> {
        Ok(sqlx::query_as(
            "SELECT * FROM te_events WHERE task_id = ? ORDER BY event_seq ASC LIMIT ?",
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    // ─── Checkpoints ──────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub async fn write_checkpoint(
        &self,
        task_id: &str,
        agent_id: &str,
        checkpoint_type: &str,
        current_action: &str,
        completed_items: &str,
        files_modified: &str,
        next_steps: &str,
        remaining_items: &str,
        context_summary: Option<&str>,
    ) -> Result<TeCheckpoint> {
        let id = new_id();
        let (seq,): (i64,) =
            sqlx::query_as("SELECT COALESCE(MAX(event_seq), 0) FROM te_events WHERE task_id = ?")
                .bind(task_id)
                .fetch_one(&self.pool)
                .await?;

        sqlx::query(
            "INSERT INTO te_checkpoints \
             (id, task_id, agent_id, checkpoint_type, current_action, completed_items, \
              files_modified, next_steps, remaining_items, context_summary, last_event_seq) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(task_id)
        .bind(agent_id)
        .bind(checkpoint_type)
        .bind(current_action)
        .bind(completed_items)
        .bind(files_modified)
        .bind(next_steps)
        .bind(remaining_items)
        .bind(context_summary)
        .bind(seq)
        .execute(&self.pool)
        .await?;

        Ok(sqlx::query_as("SELECT * FROM te_checkpoints WHERE id = ?")
            .bind(&id)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn latest_checkpoint(&self, task_id: &str) -> Result<Option<TeCheckpoint>> {
        Ok(sqlx::query_as(
            "SELECT * FROM te_checkpoints WHERE task_id = ? ORDER BY timestamp DESC LIMIT 1",
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    // ─── Notes ────────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub async fn add_note(
        &self,
        task_id: &str,
        agent_id: Option<&str>,
        note_type: &str,
        title: &str,
        content: &str,
        related_file: Option<&str>,
        visibility: &str,
    ) -> Result<TeNote> {
        let id = new_id();
        sqlx::query(
            "INSERT INTO te_notes \
             (id, task_id, agent_id, note_type, title, content, related_file, visibility) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(task_id)
        .bind(agent_id)
        .bind(note_type)
        .bind(title)
        .bind(content)
        .bind(related_file)
        .bind(visibility)
        .execute(&self.pool)
        .await?;

        Ok(sqlx::query_as("SELECT * FROM te_notes WHERE id = ?")
            .bind(&id)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn list_notes(&self, task_id: &str) -> Result<Vec<TeNote>> {
        Ok(
            sqlx::query_as("SELECT * FROM te_notes WHERE task_id = ? ORDER BY timestamp ASC")
                .bind(task_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }
}

fn unixepoch() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn task_status_to_event(status: &str) -> &str {
    match status {
        "ready" => "ready",
        "queued" => "queued",
        "claimed" => "claimed",
        "active" => "started",
        "paused" => "paused",
        "blocked" => "blocked",
        "needs_review" => "needs_review",
        "in_review" => "review_started",
        "needs_qa" => "needs_qa",
        "in_qa" => "qa_started",
        "needs_secondary" => "needs_secondary",
        "done" => "done",
        "canceled" => "canceled",
        "failed" => "failed",
        _ => "updated",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_storage() -> TaskEngineStorage {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let sqls: [&str; 2] = [
            include_str!("../storage/migrations/001_init.sql"),
            include_str!("../storage/migrations/006_task_engine.sql"),
        ];
        for sql in sqls {
            for stmt in sql.split(';') {
                let stmt: &str = stmt.trim();
                if !stmt.is_empty() {
                    let _ = sqlx::query(stmt).execute(&pool).await;
                }
            }
        }
        TaskEngineStorage::new(pool)
    }

    #[tokio::test]
    async fn test_create_phase_and_task() {
        let s = test_storage().await;
        let phase = s
            .create_phase("P99", "Test Phase", "desc", "high", None, Some("apps"))
            .await
            .unwrap();
        assert_eq!(phase.display_id, "P99");
        assert_eq!(phase.status, "planned");

        let task = s
            .create_task(
                "P99.T01",
                &phase.id,
                None,
                "Test Task",
                "desc",
                "implementation",
                "medium",
            )
            .await
            .unwrap();
        assert_eq!(task.display_id, "P99.T01");
        assert_eq!(task.status, "planned");
    }

    #[tokio::test]
    async fn test_claim_task() {
        let s = test_storage().await;
        let phase = s
            .create_phase("P98", "Phase", "d", "medium", None, None)
            .await
            .unwrap();
        let task = s
            .create_task(
                "P98.T01",
                &phase.id,
                None,
                "Task",
                "d",
                "implementation",
                "high",
            )
            .await
            .unwrap();
        let agent = s
            .register_agent("test-agent", "claude", "implementer", "[]", None, None)
            .await
            .unwrap();

        // Transition to queued
        sqlx::query("UPDATE te_tasks SET status = 'queued' WHERE id = ?")
            .bind(&task.id)
            .execute(&s.pool)
            .await
            .unwrap();

        let claimed = s.claim_task(&task.id, &agent.id).await.unwrap();
        assert!(claimed.is_some());
        assert_eq!(claimed.unwrap().status, "claimed");

        // Second claim should fail
        let agent2 = s
            .register_agent("test-agent-2", "claude", "implementer", "[]", None, None)
            .await
            .unwrap();
        let claimed2 = s.claim_task(&task.id, &agent2.id).await.unwrap();
        assert!(claimed2.is_none());
    }

    #[tokio::test]
    async fn test_checkpoint_write_and_read() {
        let s = test_storage().await;
        let phase = s
            .create_phase("P97", "Phase", "d", "medium", None, None)
            .await
            .unwrap();
        let task = s
            .create_task(
                "P97.T01",
                &phase.id,
                None,
                "Task",
                "d",
                "implementation",
                "medium",
            )
            .await
            .unwrap();
        let agent = s
            .register_agent("test-agent", "claude", "implementer", "[]", None, None)
            .await
            .unwrap();

        let cp = s
            .write_checkpoint(
                &task.id,
                &agent.id,
                "periodic",
                "Writing storage.rs",
                "[]",
                "[]",
                "[]",
                "[]",
                None,
            )
            .await
            .unwrap();
        assert_eq!(cp.checkpoint_type, "periodic");

        let latest = s.latest_checkpoint(&task.id).await.unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().id, cp.id);
    }
}
