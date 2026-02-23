//! Control Thread — one per project, persistent.
//!
//! The control thread is the orchestration hub for a project. It NEVER calls
//! file-write or apply-patch tools. Its job is to plan work, create task
//! threads, track status, and request user approvals.
//!
//! There is exactly one active control thread per `project_root`. If one
//! already exists in SQLite, `get_or_create` returns it rather than
//! creating a duplicate.

use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;

use super::model::{new_thread_id, Thread, ThreadStatus, ThreadType};

/// Shared row type for SQLite thread queries (avoids type-complexity lint).
pub type ThreadRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
    String,
);

/// Handle for operating on the control thread of a specific project.
pub struct ControlThread {
    /// Absolute path to the project root.
    pub project_root: PathBuf,
    /// The underlying thread record.
    pub thread: Thread,
}

impl ControlThread {
    /// Return the existing active control thread for `project_root`, or create
    /// a new one if none exists.
    ///
    /// This is idempotent: calling it multiple times with the same path always
    /// returns the same thread (identified by `project_root` + `thread_type =
    /// 'control'` + `status != 'archived'`).
    pub async fn get_or_create(
        pool: &SqlitePool,
        project_root: &Path,
        model_config: serde_json::Value,
    ) -> Result<Thread> {
        let root_str = project_root
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("project_root is not valid UTF-8"))?;

        // Look for an existing non-archived control thread for this project.
        // We store the project root as the task_id field (repurposed as context key)
        // since control threads have no real task_id.
        let existing: Option<ThreadRow> = sqlx::query_as(
            "SELECT thread_id, thread_type, task_id, parent_thread_id,
                    status, model_config, created_at, updated_at
             FROM threads
             WHERE thread_type = 'control'
               AND task_id = ?
               AND status != 'archived'
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(root_str)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = existing {
            return row_to_thread(row);
        }

        // No existing thread — create one.
        let thread_id = new_thread_id();
        let now = Utc::now().to_rfc3339();
        let model_json = serde_json::to_string(&model_config).unwrap_or_else(|_| "{}".to_string());

        sqlx::query(
            "INSERT INTO threads
                 (thread_id, thread_type, task_id, parent_thread_id,
                  status, model_config, created_at, updated_at)
             VALUES (?, 'control', ?, NULL, 'active', ?, ?, ?)",
        )
        .bind(&thread_id)
        .bind(root_str) // task_id = project_root for control threads
        .bind(&model_json)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        let row: ThreadRow = sqlx::query_as(
            "SELECT thread_id, thread_type, task_id, parent_thread_id,
                        status, model_config, created_at, updated_at
                 FROM threads WHERE thread_id = ?",
        )
        .bind(&thread_id)
        .fetch_one(pool)
        .await?;

        row_to_thread(row)
    }
}

/// Convert a raw SQLite row tuple into a `Thread`.
pub fn row_to_thread(row: ThreadRow) -> Result<Thread> {
    let (
        thread_id,
        thread_type_str,
        task_id,
        parent_thread_id,
        status_str,
        model_config_str,
        created_at_str,
        updated_at_str,
    ) = row;

    let thread_type = match thread_type_str.as_str() {
        "control" => ThreadType::Control,
        "task" => ThreadType::Task,
        "sub" => ThreadType::Sub,
        other => return Err(anyhow::anyhow!("unknown thread_type: {}", other)),
    };

    let status = match status_str.as_str() {
        "active" => ThreadStatus::Active,
        "paused" => ThreadStatus::Paused,
        "completed" => ThreadStatus::Completed,
        "archived" => ThreadStatus::Archived,
        "error" => ThreadStatus::Error,
        other => return Err(anyhow::anyhow!("unknown thread status: {}", other)),
    };

    let model_config: serde_json::Value =
        serde_json::from_str(&model_config_str).unwrap_or(serde_json::json!({}));

    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    Ok(Thread {
        thread_id,
        thread_type,
        task_id,
        parent_thread_id,
        status,
        model_config,
        created_at,
        updated_at,
    })
}
