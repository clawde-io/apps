//! RPC handlers for worktree management.
//!
//! Exposes:
//!   `worktrees.list`    — list all tracked task worktrees
//!   `worktrees.merge`   — merge a Done task's worktree into main
//!   `worktrees.cleanup` — remove empty/stale Done worktrees

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

fn sv<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|v| v.as_str())
}

/// `worktrees.list` — list all tracked task worktrees with their status.
///
/// Params: (none required)
/// Returns: `{ worktrees: [ { task_id, worktree_path, branch, repo_path, created_at, status } ] }`
pub async fn list(_params: Value, ctx: &AppContext) -> Result<Value> {
    let entries = ctx.worktree_manager.list().await;
    let result: Vec<Value> = entries
        .into_iter()
        .map(|w| {
            json!({
                "task_id": w.task_id,
                "worktree_path": w.worktree_path.to_string_lossy(),
                "branch": w.branch,
                "repo_path": w.repo_path.to_string_lossy(),
                "created_at": w.created_at.to_rfc3339(),
                "status": w.status,
            })
        })
        .collect();

    Ok(json!({ "worktrees": result }))
}

/// `worktrees.merge` — merge a Done task's worktree into the main branch.
///
/// Params: `{ task_id: string }`
/// Returns: `{ merged: true, task_id: string }`
///
/// The task must already be in `Done` state. This does NOT auto-approve —
/// the caller is responsible for QA gating before invoking this method.
pub async fn merge(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id = sv(&params, "task_id")
        .ok_or_else(|| anyhow::anyhow!("missing field: task_id"))?;

    crate::worktree::merge::merge_to_main(&ctx.worktree_manager, task_id).await?;

    Ok(json!({ "merged": true, "task_id": task_id }))
}

/// `worktrees.cleanup` — remove empty Done worktrees.
///
/// Params: (none required)
/// Returns: `{ removed: N }` — number of worktrees cleaned up
pub async fn cleanup(_params: Value, ctx: &AppContext) -> Result<Value> {
    let removed =
        crate::worktree::cleanup::cleanup_empty_worktrees(&ctx.worktree_manager).await?;

    Ok(json!({ "removed": removed }))
}
