//! RPC handlers for worktree management.
//!
//! Exposes:
//!   `worktrees.list`    — list all tracked task worktrees
//!   `worktrees.merge`   — merge a Done task's worktree into main
//!   `worktrees.cleanup` — remove empty/stale Done worktrees
//!   `worktrees.diff`    — get the unified diff of uncommitted changes in a task worktree

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
    let task_id =
        sv(&params, "task_id").ok_or_else(|| anyhow::anyhow!("missing field: task_id"))?;

    crate::worktree::merge::merge_to_main(&ctx.worktree_manager, task_id).await?;

    Ok(json!({ "merged": true, "task_id": task_id }))
}

/// `worktrees.cleanup` — remove empty Done worktrees.
///
/// Params: (none required)
/// Returns: `{ removed: N }` — number of worktrees cleaned up
pub async fn cleanup(_params: Value, ctx: &AppContext) -> Result<Value> {
    let removed = crate::worktree::cleanup::cleanup_empty_worktrees(&ctx.worktree_manager).await?;

    Ok(json!({ "removed": removed }))
}

/// `worktrees.diff` — get the unified diff of uncommitted changes in a task worktree.
///
/// Params: `{ task_id: string }`
/// Returns: `{ task_id, diff: string, stats: { files_changed, insertions, deletions } }`
pub async fn diff(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id =
        sv(&params, "task_id").ok_or_else(|| anyhow::anyhow!("missing field: task_id"))?;

    let info = ctx
        .worktree_manager
        .get(task_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("REPO_NOT_FOUND: no worktree for task '{}'", task_id))?;

    let wt_path = info.worktree_path.clone();
    let task_id_owned = task_id.to_string();

    let (patch_text, files_changed, insertions, deletions) =
        tokio::task::spawn_blocking(move || -> anyhow::Result<(String, usize, usize, usize)> {
            let repo = git2::Repository::open(&wt_path)
                .map_err(|e| anyhow::anyhow!("failed to open worktree: {}", e))?;

            // Diff HEAD vs working directory + index.
            let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
            let git_diff = match head_tree {
                Some(ref tree) => repo
                    .diff_tree_to_workdir_with_index(Some(tree), None)
                    .map_err(|e| anyhow::anyhow!("diff failed: {}", e))?,
                None => repo
                    .diff_index_to_workdir(None, None)
                    .map_err(|e| anyhow::anyhow!("diff (no HEAD) failed: {}", e))?,
            };

            let stats = git_diff
                .stats()
                .map_err(|e| anyhow::anyhow!("diff stats failed: {}", e))?;

            let mut patch_text = String::new();
            git_diff
                .print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
                    let origin = line.origin();
                    if let Ok(content) = std::str::from_utf8(line.content()) {
                        match origin {
                            '+' | '-' | ' ' => {
                                patch_text.push(origin);
                                patch_text.push_str(content);
                            }
                            _ => {
                                // File headers, hunk headers — include as-is
                                patch_text.push_str(content);
                            }
                        }
                    }
                    true
                })
                .map_err(|e| anyhow::anyhow!("diff format failed: {}", e))?;

            Ok((
                patch_text,
                stats.files_changed(),
                stats.insertions(),
                stats.deletions(),
            ))
        })
        .await
        .map_err(|e| anyhow::anyhow!("worktree diff task panicked: {}", e))??;

    Ok(json!({
        "task_id": task_id_owned,
        "diff": patch_text,
        "stats": {
            "files_changed": files_changed,
            "insertions": insertions,
            "deletions": deletions,
        }
    }))
}
