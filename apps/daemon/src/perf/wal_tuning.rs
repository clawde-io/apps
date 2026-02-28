// SPDX-License-Identifier: MIT
//! SQLite WAL tuning helpers — Sprint Z, SC2.T04–SC2.T05.
//!
//! Applies a set of `PRAGMA` statements that improve throughput and durability
//! for the daemon's SQLite database, which is already opened in WAL mode.
//!
//! These are applied once at startup after the connection pool is created.
//! They do NOT change the durability guarantees of WAL mode — `synchronous`
//! is kept at NORMAL (the WAL journal provides crash safety at this level).
//!
//! # References
//! - <https://www.sqlite.org/pragma.html>
//! - <https://www.sqlite.org/wal.html>
//! - <https://www.sqlite.org/rowidtable.html>

use anyhow::Result;
use sqlx::SqlitePool;
use tracing::info;

// ─── Tuning parameters ────────────────────────────────────────────────────────

/// Target page cache size in KiB.  Default SQLite is 2,000 pages × 4 KiB = ~8 MB.
/// We increase this to 32 MB to reduce disk I/O on the read-heavy session and
/// message tables.
const CACHE_SIZE_KB: i64 = 32_768; // 32 MiB

/// Maximum number of WAL frames between automatic checkpoints.
/// SQLite's default is 1000 pages; we raise it to reduce checkpoint interruptions
/// during heavy write bursts (e.g. bulk task import).
const WAL_AUTOCHECKPOINT_PAGES: i64 = 4096;

/// Memory map size in bytes.  Enables `mmap` for read paths on operating
/// systems that support it (macOS, Linux).  0 = disabled (safe default).
const MMAP_SIZE_BYTES: i64 = 128 * 1024 * 1024; // 128 MiB

/// Busy timeout in milliseconds.  If a writer holds the lock, a reader will
/// spin for up to this duration before returning SQLITE_BUSY.
const BUSY_TIMEOUT_MS: i64 = 5_000;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Apply the full set of performance PRAGMAs to the connection pool.
///
/// Should be called once after the pool is opened and migrations have run.
///
/// # Errors
///
/// Returns an error if any PRAGMA fails to execute — this is typically a sign
/// of a corrupt database or a version mismatch, so the daemon should abort.
pub async fn apply_wal_tuning(pool: &SqlitePool) -> Result<()> {
    // SQLite PRAGMAs must be executed on a single acquired connection because
    // some (like `wal_autocheckpoint`) are per-connection state.  We grab one
    // connection and run all PRAGMAs sequentially.
    let mut conn = pool.acquire().await?;

    // WAL mode — must already be set by the migration; this is a no-op if set.
    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&mut *conn)
        .await?;

    // synchronous = NORMAL is safe under WAL: a crash can corrupt at most the
    // last transaction, which is rolled back on next open.  FULL adds an
    // extra fsync per transaction and is only needed for absolute durability
    // (e.g. financial records).
    sqlx::query("PRAGMA synchronous = NORMAL")
        .execute(&mut *conn)
        .await?;

    // Negative value = set cache size in kibibytes.
    let cache_pragma = format!("PRAGMA cache_size = -{}", CACHE_SIZE_KB);
    sqlx::query(&cache_pragma).execute(&mut *conn).await?;

    // Enable write-ahead log checkpointing control.
    let wal_pragma = format!("PRAGMA wal_autocheckpoint = {}", WAL_AUTOCHECKPOINT_PAGES);
    sqlx::query(&wal_pragma).execute(&mut *conn).await?;

    // Memory-mapped I/O — safe to set; falls back silently if unsupported.
    let mmap_pragma = format!("PRAGMA mmap_size = {}", MMAP_SIZE_BYTES);
    sqlx::query(&mmap_pragma).execute(&mut *conn).await?;

    // Busy handler — retry on SQLITE_BUSY rather than failing immediately.
    let busy_pragma = format!("PRAGMA busy_timeout = {}", BUSY_TIMEOUT_MS);
    sqlx::query(&busy_pragma).execute(&mut *conn).await?;

    // Temp tables and indices in memory instead of on-disk temp files.
    sqlx::query("PRAGMA temp_store = MEMORY")
        .execute(&mut *conn)
        .await?;

    info!(
        cache_kb = CACHE_SIZE_KB,
        wal_autocheckpoint = WAL_AUTOCHECKPOINT_PAGES,
        mmap_mb = MMAP_SIZE_BYTES / (1024 * 1024),
        busy_timeout_ms = BUSY_TIMEOUT_MS,
        "SQLite WAL tuning applied"
    );

    Ok(())
}

/// Trigger a WAL checkpoint manually.
///
/// Call this on a clean shutdown to flush the WAL journal to the main database
/// file.  This reduces startup time on the next run (no WAL replay needed).
///
/// `mode` is one of `"PASSIVE"`, `"FULL"`, `"RESTART"`, or `"TRUNCATE"`.
/// Use `"TRUNCATE"` for clean shutdown — it checkpoints and resets the WAL
/// to zero bytes.
pub async fn checkpoint_wal(pool: &SqlitePool, mode: &str) -> Result<WalCheckpointResult> {
    let valid_modes = ["PASSIVE", "FULL", "RESTART", "TRUNCATE"];
    if !valid_modes.contains(&mode) {
        return Err(anyhow::anyhow!(
            "invalid WAL checkpoint mode '{}' — must be one of {:?}",
            mode,
            valid_modes
        ));
    }

    let pragma = format!("PRAGMA wal_checkpoint({})", mode);
    let row: (i64, i64, i64) = sqlx::query_as(&pragma).fetch_one(pool).await?;

    let result = WalCheckpointResult {
        busy: row.0 != 0,
        log_frames: row.1,
        checkpointed_frames: row.2,
    };

    info!(
        mode = %mode,
        busy = result.busy,
        log_frames = result.log_frames,
        checkpointed = result.checkpointed_frames,
        "WAL checkpoint complete"
    );

    Ok(result)
}

/// Result from a `PRAGMA wal_checkpoint` call.
#[derive(Debug, Clone)]
pub struct WalCheckpointResult {
    /// `true` if the checkpoint could not fully complete because a read
    /// transaction held the WAL file open.
    pub busy: bool,
    /// Total number of frames in the WAL log.
    pub log_frames: i64,
    /// Number of frames that were actually checkpointed.
    pub checkpointed_frames: i64,
}

/// Run a quick integrity check on the database.
///
/// Checks up to `max_errors` errors.  Returns `Ok(())` if the check passes,
/// or an error listing the first problems found.
pub async fn integrity_check(pool: &SqlitePool, max_errors: i64) -> Result<()> {
    let pragma = format!("PRAGMA integrity_check({})", max_errors.max(1));
    let rows: Vec<(String,)> = sqlx::query_as(&pragma).fetch_all(pool).await?;

    let issues: Vec<String> = rows
        .into_iter()
        .map(|(s,)| s)
        .filter(|s| s != "ok")
        .collect();

    if issues.is_empty() {
        info!("SQLite integrity check: ok");
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "SQLite integrity check found {} issue(s): {}",
            issues.len(),
            issues.join("; ")
        ))
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Create an in-memory SQLite pool for testing.
    async fn test_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("failed to open in-memory SQLite")
    }

    #[tokio::test]
    async fn apply_wal_tuning_succeeds_on_in_memory_db() {
        let pool = test_pool().await;
        // WAL mode is not fully supported by in-memory SQLite (returns "memory"),
        // but all other PRAGMAs should execute without error.
        let result = apply_wal_tuning(&pool).await;
        assert!(result.is_ok(), "apply_wal_tuning failed: {:?}", result);
    }

    #[tokio::test]
    async fn checkpoint_wal_invalid_mode_returns_error() {
        let pool = test_pool().await;
        let result = checkpoint_wal(&pool, "INVALID_MODE").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("invalid WAL checkpoint mode"));
    }

    #[tokio::test]
    async fn checkpoint_wal_valid_modes_accepted() {
        let pool = test_pool().await;
        // In-memory SQLite does not maintain a WAL file, so the checkpoint
        // itself may be a no-op, but the PRAGMA should execute without error.
        for mode in ["PASSIVE", "FULL", "RESTART", "TRUNCATE"] {
            let result = checkpoint_wal(&pool, mode).await;
            assert!(
                result.is_ok(),
                "checkpoint_wal({mode}) failed: {:?}",
                result
            );
        }
    }

    #[tokio::test]
    async fn integrity_check_passes_on_fresh_db() {
        let pool = test_pool().await;
        let result = integrity_check(&pool, 10).await;
        assert!(result.is_ok(), "integrity_check failed: {:?}", result);
    }

    #[test]
    fn wal_checkpoint_result_fields() {
        let r = WalCheckpointResult {
            busy: false,
            log_frames: 100,
            checkpointed_frames: 100,
        };
        assert!(!r.busy);
        assert_eq!(r.log_frames, r.checkpointed_frames);
    }
}
