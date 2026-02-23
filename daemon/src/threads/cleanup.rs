//! Thread archival and cleanup (Phase 43f).
//!
//! Control threads persist indefinitely — they are the project's long-running
//! orchestrator. Task threads, however, are ephemeral: once a task completes
//! (or errors), the thread can be archived to keep the `threads` table lean.
//!
//! Archiving sets `status = 'archived'` and `updated_at = now()`.
//! Rows are NOT deleted — they serve as an audit trail.

use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::info;

/// Archive completed (and optionally error'd) task threads older than
/// `older_than`.
///
/// Only `thread_type IN ('task', 'sub')` threads are archived — control
/// threads persist forever.
///
/// Returns the number of threads updated.
pub async fn archive_completed_threads(
    pool: &SqlitePool,
    older_than: Duration,
) -> Result<u64> {
    let cutoff = Utc::now()
        - chrono::Duration::from_std(older_than)
            .map_err(|e| anyhow::anyhow!("duration conversion failed: {e}"))?;
    let cutoff_str = cutoff.to_rfc3339();
    let now = Utc::now().to_rfc3339();

    let result = sqlx::query(
        "UPDATE threads
         SET status = 'archived', updated_at = ?
         WHERE thread_type IN ('task', 'sub')
           AND status IN ('completed', 'error')
           AND updated_at < ?",
    )
    .bind(&now)
    .bind(&cutoff_str)
    .execute(pool)
    .await?;

    let count = result.rows_affected();
    if count > 0 {
        info!(count, older_than_secs = older_than.as_secs(), "archived completed task threads");
    }

    Ok(count)
}

/// Archive a single thread by ID, regardless of age.
///
/// Used when a task is explicitly cancelled or superseded.
/// No-ops silently if the thread is already archived.
pub async fn archive_thread(pool: &SqlitePool, thread_id: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE threads
         SET status = 'archived', updated_at = ?
         WHERE thread_id = ? AND status != 'archived'",
    )
    .bind(&now)
    .bind(thread_id)
    .execute(pool)
    .await?;
    Ok(())
}
