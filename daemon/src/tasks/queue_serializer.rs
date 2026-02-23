/// Serializes the `agent_tasks` DB table to `tasks/queue.json`.
/// Written atomically: tmp file → rename to prevent partial reads.

use super::storage::{AgentTaskRow, TaskStorage};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;

pub async fn flush_queue(storage: &TaskStorage, repo_path: &str) -> Result<()> {
    let queue_path = Path::new(repo_path)
        .join(".claude")
        .join("tasks")
        .join("queue.json");

    let tasks = storage
        .list_tasks(&super::storage::TaskListParams {
            repo_path: Some(repo_path.to_string()),
            ..Default::default()
        })
        .await?;

    let now = chrono::Utc::now().to_rfc3339();
    let payload = json!({
        "version": 1,
        "repo_path": repo_path,
        "updated_at": now,
        "tasks": tasks.iter().map(task_to_json).collect::<Vec<Value>>()
    });

    let json_str = serde_json::to_string_pretty(&payload)?;

    // Ensure directory exists
    if let Some(parent) = queue_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Atomic write: write to tmp, then rename
    let tmp_path = queue_path.with_extension("json.tmp");
    fs::write(&tmp_path, json_str).await?;
    fs::rename(&tmp_path, &queue_path).await?;

    Ok(())
}

fn task_to_json(t: &AgentTaskRow) -> Value {
    json!({
        "id": t.id,
        "title": t.title,
        "type": t.task_type,
        "phase": t.phase,
        "group": t.group,
        "parent_id": t.parent_id,
        "severity": t.severity,
        "status": t.status,
        "claimed_by": t.claimed_by,
        "claimed_at": t.claimed_at,
        "started_at": t.started_at,
        "completed_at": t.completed_at,
        "last_heartbeat": t.last_heartbeat,
        "file": t.file,
        "files": t.files.as_deref().and_then(|s| serde_json::from_str::<Value>(s).ok()).unwrap_or(json!([])),
        "depends_on": t.depends_on.as_deref().and_then(|s| serde_json::from_str::<Value>(s).ok()).unwrap_or(json!([])),
        "blocks": t.blocks.as_deref().and_then(|s| serde_json::from_str::<Value>(s).ok()).unwrap_or(json!([])),
        "tags": t.tags.as_deref().and_then(|s| serde_json::from_str::<Value>(s).ok()).unwrap_or(json!([])),
        "notes": t.notes,
        "block_reason": t.block_reason,
        "estimated_minutes": t.estimated_minutes,
        "actual_minutes": t.actual_minutes,
        "repo_path": t.repo_path,
        "created_at": t.created_at,
        "updated_at": t.updated_at
    })
}
