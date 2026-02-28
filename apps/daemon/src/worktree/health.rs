//! Periodic worktree health check.
//!
//! Scans all tracked worktrees for orphaned entries (task no longer active)
//! and stale entries (Done for more than 24 hours without cleanup).

use anyhow::Result;
use chrono::{Duration, Utc};

use super::manager::{WorktreeManager, WorktreeStatus};

/// Health report for all tracked worktrees.
#[derive(Debug)]
pub struct WorktreeHealth {
    /// task_ids whose task record is no longer active (orphaned).
    pub orphaned: Vec<String>,
    /// task_ids that have been in Done state for more than 24 hours.
    pub stale: Vec<String>,
    /// Total number of tracked worktrees.
    pub total: usize,
}

/// Scan all worktrees and report health.
///
/// A worktree is *stale* when it has been `Done` for more than 24 hours.
/// A worktree is *orphaned* when its directory no longer exists on disk
/// (the worktree was cleaned up externally or the data dir changed).
pub async fn check_health(manager: &WorktreeManager) -> Result<WorktreeHealth> {
    let worktrees = manager.list().await;
    let total = worktrees.len();
    let now = Utc::now();
    let stale_threshold = Duration::hours(24);

    let mut orphaned = Vec::new();
    let mut stale = Vec::new();

    for info in &worktrees {
        // Orphaned: directory is gone but entry still tracked.
        if !info.worktree_path.exists() && info.status == WorktreeStatus::Active {
            orphaned.push(info.task_id.clone());
            continue;
        }

        // Stale: Done for > 24 hours.
        if info.status == WorktreeStatus::Done {
            let age = now.signed_duration_since(info.created_at);
            if age > stale_threshold {
                stale.push(info.task_id.clone());
            }
        }
    }

    Ok(WorktreeHealth {
        orphaned,
        stale,
        total,
    })
}
