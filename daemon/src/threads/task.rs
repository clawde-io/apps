//! Task Thread — scoped to one task, runs in a git worktree.
//!
//! Task threads are ISOLATED from control thread history. Their context is
//! seeded from the task spec and relevant repo state only — they never see
//! the full control conversation. This prevents context bleed between tasks.

use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;

use super::{
    control::row_to_thread,
    model::{new_thread_id, Thread},
};

/// Factory for task-scoped conversation threads.
pub struct TaskThread;

impl TaskThread {
    /// Create a new task thread in SQLite, linked to `task_id` and
    /// optionally forked from `parent_thread_id`.
    ///
    /// The new thread starts in `active` status.
    pub async fn create(
        pool: &SqlitePool,
        task_id: &str,
        parent_thread_id: Option<&str>,
        model_config: serde_json::Value,
    ) -> Result<Thread> {
        let thread_id = new_thread_id();
        let now = Utc::now().to_rfc3339();
        let model_json = serde_json::to_string(&model_config)
            .unwrap_or_else(|_| "{}".to_string());

        sqlx::query(
            "INSERT INTO threads
                 (thread_id, thread_type, task_id, parent_thread_id,
                  status, model_config, created_at, updated_at)
             VALUES (?, 'task', ?, ?, 'active', ?, ?, ?)",
        )
        .bind(&thread_id)
        .bind(task_id)
        .bind(parent_thread_id)
        .bind(&model_json)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        let row: (String, String, Option<String>, Option<String>, String, String, String, String) =
            sqlx::query_as(
                "SELECT thread_id, thread_type, task_id, parent_thread_id,
                        status, model_config, created_at, updated_at
                 FROM threads WHERE thread_id = ?",
            )
            .bind(&thread_id)
            .fetch_one(pool)
            .await?;

        row_to_thread(row)
    }

    /// Build the initial OpenAI-compatible messages array that seeds a task
    /// thread with everything it needs to start working.
    ///
    /// Task threads receive ONLY:
    ///   1. A system prompt with task goal and rules
    ///   2. The task spec (title, acceptance criteria, test plan, repo path)
    ///   3. Relevant file snapshots at the time of seed
    ///
    /// They do NOT inherit control thread conversation history.
    pub fn seed_context(
        task_spec: &serde_json::Value,
        repo_state: &str,
    ) -> Vec<serde_json::Value> {
        let title = task_spec
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed task");

        let summary = task_spec
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let acceptance = task_spec
            .get("acceptance_criteria")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| format!("- {}", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        let test_plan = task_spec
            .get("test_plan")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let system_content = format!(
            "You are a task-scoped AI agent working on a specific development task.\n\
             You are running inside a git worktree isolated from other work.\n\
             You MUST complete ONLY the task described below — no scope creep.\n\
             When done, summarise the changes made and stop.\n\n\
             ## Task: {title}\n\
             {summary}\n\n\
             ## Acceptance Criteria\n\
             {acceptance}\n\n\
             ## Test Plan\n\
             {test_plan}"
        );

        let repo_content = format!(
            "## Current Repository State\n\n```\n{repo_state}\n```"
        );

        vec![
            serde_json::json!({ "role": "system", "content": system_content }),
            serde_json::json!({ "role": "user",   "content": repo_content }),
        ]
    }
}
