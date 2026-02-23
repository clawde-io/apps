//! Rule-change detection and background file watcher.
//!
//! Compares the current hash of `.claw/policies/` against the saved version.
//! A background task can poll for changes and log a warning when policies drift.

use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::warn;

use super::versioning::{hash_policy_dir, load_rule_version, save_rule_version};

// ─── Detection ───────────────────────────────────────────────────────────────

/// Returns `true` if the policies directory has changed since the last call to
/// `save_rule_version`.  Also saves the new hash when a change is detected.
pub async fn rules_changed(data_dir: &Path) -> Result<bool> {
    let policies_dir = data_dir.join("policies");
    let current = hash_policy_dir(&policies_dir).await?;

    if current.is_empty() {
        // No policies dir — nothing to compare.
        return Ok(false);
    }

    let saved = load_rule_version(data_dir).await?;

    match saved {
        None => {
            // First run — save current hash and report no change.
            save_rule_version(data_dir, &current).await?;
            Ok(false)
        }
        Some(ref prev) if prev == &current => Ok(false),
        Some(_) => {
            // Hash changed — persist new version and report the change.
            save_rule_version(data_dir, &current).await?;
            Ok(true)
        }
    }
}

// ─── Background watcher ───────────────────────────────────────────────────────

/// Start a background task that polls for policy file changes every 60 seconds.
///
/// Logs a `WARN` when changes are detected so operators can review updated rules
/// and re-run evals.  The task runs indefinitely; it stops when the daemon exits.
pub async fn watch_rules(data_dir: PathBuf) -> Result<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.tick().await; // skip immediate tick

        loop {
            interval.tick().await;
            match rules_changed(&data_dir).await {
                Ok(true) => {
                    warn!("evals: policy files have changed — re-run evals to validate new rules");
                }
                Ok(false) => {}
                Err(e) => {
                    tracing::debug!(err = %e, "evals: error checking rule changes");
                }
            }
        }
    });
    Ok(())
}
