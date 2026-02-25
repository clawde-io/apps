// SPDX-License-Identifier: MIT
//! RPC handlers for worktree management.
//!
//! Exposes:
//!   `worktrees.create`  — create a new git worktree for a task
//!   `worktrees.list`    — list all tracked task worktrees
//!   `worktrees.diff`    — get the unified diff of uncommitted changes in a task worktree
//!   `worktrees.commit`  — stage all changes and create a commit in a task worktree
//!   `worktrees.accept`  — squash-merge a worktree into the main branch (accept changes)
//!   `worktrees.reject`  — delete a worktree and discard its changes (reject changes)
//!   `worktrees.delete`  — hard-delete a worktree (admin/cleanup)
//!   `worktrees.merge`   — (legacy) merge a Done task worktree into main
//!   `worktrees.cleanup` — (legacy) remove empty/stale Done worktrees

use crate::worktree::manager::WorktreeStatus;
use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

fn sv<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|v| v.as_str())
}

/// `worktrees.create` — create a new git worktree for `task_id`.
///
/// Params: `{ task_id: string, task_title: string, repo_path: string }`
/// Returns: `{ task_id, worktree_path, branch, repo_path, status, created_at }`
/// Push event: `worktree.created { task_id, worktree_path, branch }`
pub async fn create(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id =
        sv(&params, "task_id").ok_or_else(|| anyhow::anyhow!("missing field: task_id"))?;
    let task_title = sv(&params, "task_title").unwrap_or(task_id);
    let repo_path_str =
        sv(&params, "repo_path").ok_or_else(|| anyhow::anyhow!("missing field: repo_path"))?;

    // Validate repo path exists.
    let repo_path = std::path::Path::new(repo_path_str);
    if !repo_path.exists() {
        anyhow::bail!("REPO_NOT_FOUND: repo_path does not exist: {}", repo_path_str);
    }

    // Reject if worktree already exists for this task.
    if ctx.worktree_manager.get(task_id).await.is_some() {
        anyhow::bail!("worktree already exists for task '{}'", task_id);
    }

    // Create via manager (does the git work).
    let info = ctx
        .worktree_manager
        .create(task_id, task_title, repo_path)
        .await?;

    // Persist to DB.
    ctx.storage
        .create_worktree(
            task_id,
            &info.worktree_path.to_string_lossy(),
            &info.branch,
            repo_path_str,
        )
        .await?;

    // Broadcast push event.
    ctx.broadcaster.broadcast(
        "worktree.created",
        json!({
            "taskId": task_id,
            "worktreePath": info.worktree_path.to_string_lossy(),
            "branch": info.branch,
        }),
    );

    Ok(json!({
        "task_id": info.task_id,
        "worktree_path": info.worktree_path.to_string_lossy(),
        "branch": info.branch,
        "repo_path": info.repo_path.to_string_lossy(),
        "status": info.status,
        "created_at": info.created_at.to_rfc3339(),
    }))
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

/// `worktrees.commit` — stage all changes and create a commit in a task worktree.
///
/// Params: `{ task_id: string, message: string }`
/// Returns: `{ task_id, sha: string }`
pub async fn commit(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id =
        sv(&params, "task_id").ok_or_else(|| anyhow::anyhow!("missing field: task_id"))?;
    let message = sv(&params, "message")
        .unwrap_or("task commit")
        .to_string();

    let info = ctx
        .worktree_manager
        .get(task_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("REPO_NOT_FOUND: no worktree for task '{}'", task_id))?;

    let wt_path = info.worktree_path.clone();
    let task_id_owned = task_id.to_string();
    let commit_msg = message.clone();

    let sha = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        let repo = git2::Repository::open(&wt_path)
            .map_err(|e| anyhow::anyhow!("failed to open worktree: {}", e))?;

        // Stage all changes (git add -A equivalent).
        let mut index = repo
            .index()
            .map_err(|e| anyhow::anyhow!("failed to get index: {}", e))?;
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| anyhow::anyhow!("failed to stage files: {}", e))?;
        index
            .write()
            .map_err(|e| anyhow::anyhow!("failed to write index: {}", e))?;

        let tree_oid = index
            .write_tree()
            .map_err(|e| anyhow::anyhow!("failed to write tree: {}", e))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| anyhow::anyhow!("failed to find tree: {}", e))?;
        let sig = repo
            .signature()
            .map_err(|e| anyhow::anyhow!("failed to get signature: {}", e))?;

        let parents: Vec<git2::Commit<'_>> = if let Ok(head) = repo.head() {
            if let Ok(c) = head.peel_to_commit() {
                vec![c]
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();

        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, &commit_msg, &tree, &parent_refs)
            .map_err(|e| anyhow::anyhow!("commit failed: {}", e))?;

        Ok(oid.to_string())
    })
    .await
    .map_err(|e| anyhow::anyhow!("worktree commit task panicked: {}", e))??;

    Ok(json!({
        "task_id": task_id_owned,
        "sha": sha,
    }))
}

/// `worktrees.accept` — squash-merge a worktree into the main branch and clean up.
///
/// Params: `{ task_id: string }`
/// Returns: `{ task_id, merged: true }`
/// Push event: `worktree.accepted { taskId, branch }`
pub async fn accept(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id =
        sv(&params, "task_id").ok_or_else(|| anyhow::anyhow!("missing field: task_id"))?;

    let info = ctx
        .worktree_manager
        .get(task_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("REPO_NOT_FOUND: no worktree for task '{}'", task_id))?;

    // Mark as Done first (required by merge_to_main).
    ctx.worktree_manager
        .set_status(task_id, WorktreeStatus::Done)
        .await;

    let task_id_owned = task_id.to_string();
    let branch = info.branch.clone();

    // Perform the merge.
    let merge_result =
        crate::worktree::merge::merge_to_main(&ctx.worktree_manager, task_id).await;

    match merge_result {
        Ok(()) => {
            // Update DB.
            ctx.storage
                .set_worktree_status(task_id, "merged")
                .await
                .ok();

            // Broadcast accepted event.
            ctx.broadcaster.broadcast(
                "worktree.accepted",
                json!({
                    "taskId": task_id_owned,
                    "branch": branch,
                }),
            );

            Ok(json!({
                "task_id": task_id_owned,
                "merged": true,
            }))
        }
        Err(e) => {
            // Revert status on failure.
            ctx.worktree_manager
                .set_status(task_id, WorktreeStatus::Active)
                .await;
            // Return conflict error with details.
            anyhow::bail!("MERGE_CONFLICT: {}", e)
        }
    }
}

/// `worktrees.reject` — delete a worktree and discard its changes.
///
/// Params: `{ task_id: string }`
/// Returns: `{ task_id, deleted: true }`
/// Push event: `worktree.rejected { taskId, branch }`
pub async fn reject(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id =
        sv(&params, "task_id").ok_or_else(|| anyhow::anyhow!("missing field: task_id"))?;

    let info = ctx
        .worktree_manager
        .get(task_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("REPO_NOT_FOUND: no worktree for task '{}'", task_id))?;

    let branch = info.branch.clone();
    let task_id_owned = task_id.to_string();

    // Remove via manager (deletes directory and git worktree).
    ctx.worktree_manager.remove(task_id).await?;

    // Update DB.
    ctx.storage
        .set_worktree_status(task_id, "abandoned")
        .await
        .ok();

    // Broadcast rejected event.
    ctx.broadcaster.broadcast(
        "worktree.rejected",
        json!({
            "taskId": task_id_owned,
            "branch": branch,
        }),
    );

    Ok(json!({
        "task_id": task_id_owned,
        "deleted": true,
    }))
}

/// `worktrees.delete` — hard-delete a worktree (admin cleanup, no merge).
///
/// Params: `{ task_id: string }`
/// Returns: `{ task_id, deleted: true }`
pub async fn delete(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id =
        sv(&params, "task_id").ok_or_else(|| anyhow::anyhow!("missing field: task_id"))?;
    let task_id_owned = task_id.to_string();

    // Remove via manager (best-effort).
    let _ = ctx.worktree_manager.remove(task_id).await;

    // Delete from DB.
    ctx.storage.delete_worktree(task_id).await.ok();

    Ok(json!({
        "task_id": task_id_owned,
        "deleted": true,
    }))
}

/// `worktrees.merge` — (legacy) merge a Done task's worktree into the main branch.
///
/// Params: `{ task_id: string }`
/// Returns: `{ merged: true, task_id: string }`
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
