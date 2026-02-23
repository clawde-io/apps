//! IPC handlers for conversation threading (Phase 43f).
//!
//! Exposes four RPC methods:
//!   - `threads.start`  — create a new thread (control, task, or sub)
//!   - `threads.resume` — fetch a thread + its latest vendor session snapshots
//!   - `threads.fork`   — fork a task thread into a new sub thread
//!   - `threads.list`   — query threads by type, status, or task_id

use crate::threads::{
    control::{ControlThread, ThreadRow},
    model::{new_thread_id, Thread},
    task::TaskThread,
};
use crate::AppContext;
use anyhow::Result;
use chrono::Utc;
use serde_json::{json, Value};

// ─── Helpers (mirrors pattern in tasks.rs) ───────────────────────────────────

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn sv<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|v| v.as_str())
}

/// Serialize a `Thread` to a `serde_json::Value` suitable for RPC responses.
fn thread_to_json(t: &Thread) -> Value {
    json!({
        "thread_id":        t.thread_id,
        "thread_type":      t.thread_type.as_str(),
        "task_id":          t.task_id,
        "parent_thread_id": t.parent_thread_id,
        "status":           t.status.as_str(),
        "model_config":     t.model_config,
        "created_at":       t.created_at.to_rfc3339(),
        "updated_at":       t.updated_at.to_rfc3339(),
    })
}

// ─── threads.start ───────────────────────────────────────────────────────────

/// `threads.start` — Create a new thread.
///
/// Params:
/// ```json
/// {
///   "thread_type":      "control" | "task" | "sub",  // required
///   "task_id":          "...",   // required for task/sub threads
///   "parent_thread_id": "...",   // required for sub threads; used for task threads too
///   "project_root":     "...",   // required for control threads
///   "model_config":     { ... }  // optional; defaults to {}
/// }
/// ```
///
/// Returns: `{ thread_id, thread_type, status, created_at }`
pub async fn start_thread(ctx: &AppContext, params: Value) -> Result<Value> {
    let thread_type_str = sv(&params, "thread_type")
        .ok_or_else(|| anyhow::anyhow!("missing thread_type"))?;

    let model_config = params
        .get("model_config")
        .cloned()
        .unwrap_or(json!({}));

    let pool = ctx.storage.pool();

    let thread = match thread_type_str {
        "control" => {
            let project_root = sv(&params, "project_root")
                .ok_or_else(|| anyhow::anyhow!("control thread requires project_root"))?;
            ControlThread::get_or_create(
                &pool,
                std::path::Path::new(project_root),
                model_config,
            )
            .await?
        }
        "task" => {
            let task_id = sv(&params, "task_id")
                .ok_or_else(|| anyhow::anyhow!("task thread requires task_id"))?;
            let parent_id = sv(&params, "parent_thread_id");
            TaskThread::create(&pool, task_id, parent_id, model_config).await?
        }
        "sub" => {
            let task_id = sv(&params, "task_id")
                .ok_or_else(|| anyhow::anyhow!("sub thread requires task_id"))?;
            let parent_id = sv(&params, "parent_thread_id")
                .ok_or_else(|| anyhow::anyhow!("sub thread requires parent_thread_id"))?;

            let thread_id = new_thread_id();
            let now = Utc::now().to_rfc3339();
            let model_json = serde_json::to_string(&model_config)
                .unwrap_or_else(|_| "{}".to_string());

            sqlx::query(
                "INSERT INTO threads
                     (thread_id, thread_type, task_id, parent_thread_id,
                      status, model_config, created_at, updated_at)
                 VALUES (?, 'sub', ?, ?, 'active', ?, ?, ?)",
            )
            .bind(&thread_id)
            .bind(task_id)
            .bind(parent_id)
            .bind(&model_json)
            .bind(&now)
            .bind(&now)
            .execute(&pool)
            .await?;

            fetch_thread(&pool, &thread_id).await?
        }
        other => return Err(anyhow::anyhow!("unknown thread_type: {}", other)),
    };

    Ok(json!({
        "thread_id":   thread.thread_id,
        "thread_type": thread.thread_type.as_str(),
        "status":      thread.status.as_str(),
        "created_at":  thread.created_at.to_rfc3339(),
    }))
}

// ─── threads.resume ──────────────────────────────────────────────────────────

/// `threads.resume` — Fetch a thread record with its latest vendor session snapshots.
///
/// Params: `{ "thread_id": "TH-abc123" }`
///
/// Returns the full thread record plus any saved vendor session IDs (for
/// AI provider session resume).
pub async fn resume_thread(ctx: &AppContext, params: Value) -> Result<Value> {
    let thread_id = sv(&params, "thread_id")
        .ok_or_else(|| anyhow::anyhow!("missing thread_id"))?;

    let pool = ctx.storage.pool();
    let thread = fetch_thread(&pool, thread_id).await?;

    // Fetch latest session snapshots for this thread (all vendors).
    let snapshots: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT vendor, vendor_session_id, model_config, snapshot_at
         FROM thread_session_snapshots
         WHERE thread_id = ?
         GROUP BY vendor
         HAVING snapshot_at = MAX(snapshot_at)",
    )
    .bind(thread_id)
    .fetch_all(&pool)
    .await?;

    let vendor_sessions: Vec<Value> = snapshots
        .into_iter()
        .map(|(vendor, vendor_session_id, model_config_str, snapshot_at)| {
            let mc: Value =
                serde_json::from_str(&model_config_str).unwrap_or(json!({}));
            json!({
                "vendor":            vendor,
                "vendor_session_id": vendor_session_id,
                "model_config":      mc,
                "snapshot_at":       snapshot_at,
            })
        })
        .collect();

    let mut result = thread_to_json(&thread);
    result
        .as_object_mut()
        .unwrap()
        .insert("vendor_sessions".to_string(), json!(vendor_sessions));

    Ok(result)
}

// ─── threads.fork ────────────────────────────────────────────────────────────

/// `threads.fork` — Fork an existing thread into a new sub thread.
///
/// Params:
/// ```json
/// { "thread_id": "TH-...", "model_config": { ... } }
/// ```
///
/// Returns: `{ new_thread_id, parent_thread_id }`
pub async fn fork_thread(ctx: &AppContext, params: Value) -> Result<Value> {
    let parent_id = sv(&params, "thread_id")
        .ok_or_else(|| anyhow::anyhow!("missing thread_id"))?;

    let pool = ctx.storage.pool();

    // Verify parent thread exists.
    let parent = fetch_thread(&pool, parent_id).await?;

    let model_config = params
        .get("model_config")
        .cloned()
        .unwrap_or_else(|| parent.model_config.clone());

    let new_thread_id = new_thread_id();
    let now = Utc::now().to_rfc3339();
    let model_json = serde_json::to_string(&model_config)
        .unwrap_or_else(|_| "{}".to_string());

    sqlx::query(
        "INSERT INTO threads
             (thread_id, thread_type, task_id, parent_thread_id,
              status, model_config, created_at, updated_at)
         VALUES (?, 'sub', ?, ?, 'active', ?, ?, ?)",
    )
    .bind(&new_thread_id)
    .bind(parent.task_id.as_deref().unwrap_or(""))
    .bind(parent_id)
    .bind(&model_json)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await?;

    Ok(json!({
        "new_thread_id":    new_thread_id,
        "parent_thread_id": parent_id,
    }))
}

// ─── threads.list ────────────────────────────────────────────────────────────

/// `threads.list` — Query threads with optional filters.
///
/// Params (all optional):
/// ```json
/// { "thread_type": "control|task|sub", "status": "active|paused|...", "task_id": "..." }
/// ```
///
/// Returns: `{ "threads": [...] }`
pub async fn list_threads(ctx: &AppContext, params: Value) -> Result<Value> {
    let pool = ctx.storage.pool();

    // Build a dynamic query. We use a fixed query with optional filters via
    // sqlx's parameterised binding (no string interpolation — no SQL injection).
    //
    // SQLite doesn't support named parameters well across all sqlx versions,
    // so we construct the WHERE clause manually and use positional ?-binds.
    let thread_type_filter = s(&params, "thread_type");
    let status_filter = s(&params, "status");
    let task_id_filter = s(&params, "task_id");

    let mut conditions: Vec<&str> = Vec::new();
    if thread_type_filter.is_some() {
        conditions.push("thread_type = ?");
    }
    if status_filter.is_some() {
        conditions.push("status = ?");
    }
    if task_id_filter.is_some() {
        conditions.push("task_id = ?");
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT thread_id, thread_type, task_id, parent_thread_id,
                status, model_config, created_at, updated_at
         FROM threads
         {where_clause}
         ORDER BY created_at DESC
         LIMIT 200"
    );

    // Bind parameters in order.
    let mut q = sqlx::query_as::<_, ThreadRow>(&sql);
    if let Some(ref v) = thread_type_filter {
        q = q.bind(v);
    }
    if let Some(ref v) = status_filter {
        q = q.bind(v);
    }
    if let Some(ref v) = task_id_filter {
        q = q.bind(v);
    }

    let rows = q.fetch_all(&pool).await?;

    let threads: Vec<Value> = rows
        .into_iter()
        .filter_map(|row| {
            crate::threads::control::row_to_thread(row)
                .ok()
                .map(|t| thread_to_json(&t))
        })
        .collect();

    Ok(json!({ "threads": threads }))
}

// ─── Private helpers ─────────────────────────────────────────────────────────

/// Fetch a single thread by ID or return an error if not found.
async fn fetch_thread(pool: &sqlx::SqlitePool, thread_id: &str) -> Result<Thread> {
    let row: Option<ThreadRow> = sqlx::query_as(
            "SELECT thread_id, thread_type, task_id, parent_thread_id,
                    status, model_config, created_at, updated_at
             FROM threads WHERE thread_id = ?",
        )
        .bind(thread_id)
        .fetch_optional(pool)
        .await?;

    match row {
        Some(r) => crate::threads::control::row_to_thread(r),
        None => Err(anyhow::anyhow!("thread not found: {}", thread_id)),
    }
}
