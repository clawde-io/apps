//! Auto-cleanup of no-change and stale worktrees.
//!
//! Worktrees for `Done` tasks with zero uncommitted changes are removed
//! automatically to prevent accumulation of stale checkout directories.

use anyhow::Result;
use tracing::{debug, warn};

use super::manager::{WorktreeManager, WorktreeStatus};

/// Remove worktrees for `Done` tasks that have zero uncommitted changes.
///
/// Uses `git2::Repository::statuses` on each worktree directory to check.
/// Returns the number of worktrees that were removed.
pub async fn cleanup_empty_worktrees(manager: &WorktreeManager) -> Result<u32> {
    let worktrees = manager.list().await;
    let mut removed = 0u32;

    for info in worktrees {
        if info.status != WorktreeStatus::Done {
            continue;
        }

        let wt_path = info.worktree_path.clone();
        let task_id = info.task_id.clone();

        // Check for uncommitted changes in a blocking task.
        let is_clean = tokio::task::spawn_blocking(move || worktree_is_clean(&wt_path))
            .await
            .unwrap_or(Ok(false))
            .unwrap_or(false);

        if is_clean {
            match manager.remove(&task_id).await {
                Ok(true) => {
                    debug!(task_id, "removed empty done worktree");
                    removed += 1;
                }
                Ok(false) => {}
                Err(e) => warn!(task_id, err = %e, "failed to remove worktree during cleanup"),
            }
        }
    }

    Ok(removed)
}

/// Returns `true` if the worktree at `wt_path` has no uncommitted changes.
fn worktree_is_clean(wt_path: &std::path::Path) -> Result<bool> {
    if !wt_path.exists() {
        // Already gone — treat as clean.
        return Ok(true);
    }
    let repo = git2::Repository::open(wt_path)?;
    let statuses = repo.statuses(None)?;
    Ok(statuses.is_empty())
}
