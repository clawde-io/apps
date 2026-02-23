//! Retention policy — prune old trace files.
//!
//! By default, rotated trace files (e.g. `traces-20260101-120000.jsonl`) older
//! than 30 days are deleted.  The active `traces.jsonl` is never deleted.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use tracing::info;

// ─── RetentionPolicy ─────────────────────────────────────────────────────────

/// Governs how long trace files are kept before pruning.
pub struct RetentionPolicy {
    /// Number of days to retain rotated trace files.  Default: 30.
    pub traces_days: u32,
    /// Whether to retain the event log indefinitely (currently no-op; always true).
    pub event_logs: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            traces_days: 30,
            event_logs: true,
        }
    }
}

// ─── Pruning ──────────────────────────────────────────────────────────────────

/// Delete rotated trace files older than `policy.traces_days`.
///
/// Returns the number of files that were deleted.  Errors from individual file
/// deletions are logged and skipped so that one bad file doesn't abort the
/// entire prune pass.
pub async fn prune_traces(data_dir: &Path, policy: &RetentionPolicy) -> Result<u32> {
    let telemetry_dir = data_dir.join("telemetry");

    if !telemetry_dir.exists() {
        return Ok(0);
    }

    let cutoff = Utc::now()
        - chrono::Duration::days(policy.traces_days as i64);

    let mut deleted: u32 = 0;

    let mut entries = tokio::fs::read_dir(&telemetry_dir)
        .await
        .with_context(|| format!("read telemetry dir: {}", telemetry_dir.display()))?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        // Only consider rotated files — skip the live `traces.jsonl`.
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name == "traces.jsonl" || !name.starts_with("traces-") {
            continue;
        }

        // Use filesystem mtime to determine age.
        let metadata = match tokio::fs::metadata(&path).await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(path = %path.display(), err = %e, "retention: could not stat file");
                continue;
            }
        };

        let modified = match metadata.modified() {
            Ok(t) => t,
            Err(_) => continue,
        };

        let modified_dt: chrono::DateTime<Utc> = modified.into();
        if modified_dt < cutoff {
            match tokio::fs::remove_file(&path).await {
                Ok(_) => {
                    info!(path = %path.display(), "retention: pruned old trace file");
                    deleted += 1;
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), err = %e, "retention: failed to delete trace file");
                }
            }
        }
    }

    Ok(deleted)
}
