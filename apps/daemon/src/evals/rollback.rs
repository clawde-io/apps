//! Policy rollback — snapshot and restore the policies directory.
//!
//! Before applying a new set of policies, callers should call `save_snapshot`
//! to preserve the current state.  If evals fail after the change, `restore_snapshot`
//! reverts to the last known-good configuration.
//!
//! Snapshots are stored under `.claw/evals/snapshots/{snapshot_id}/`.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use tracing::info;

// ─── Paths ────────────────────────────────────────────────────────────────────

fn snapshots_dir(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("evals").join("snapshots")
}

fn policies_dir(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("policies")
}

// ─── Save ─────────────────────────────────────────────────────────────────────

/// Snapshot the current policies directory.
///
/// Returns a `snapshot_id` string (timestamp-based) that can be passed to
/// `restore_snapshot` to revert.  If the policies directory does not exist, an
/// empty snapshot is created (this is not an error).
pub async fn save_snapshot(data_dir: &Path) -> Result<String> {
    let snapshot_id = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let dest = snapshots_dir(data_dir).join(&snapshot_id);

    tokio::fs::create_dir_all(&dest)
        .await
        .with_context(|| format!("create snapshot dir: {}", dest.display()))?;

    let src = policies_dir(data_dir);
    if src.exists() {
        copy_dir_recursive(&src, &dest).await?;
    }

    info!(snapshot_id = %snapshot_id, "evals: saved policy snapshot");
    Ok(snapshot_id)
}

// ─── Restore ─────────────────────────────────────────────────────────────────

/// Restore a previously saved snapshot to the policies directory.
///
/// The current policies directory is overwritten.  Use `save_snapshot` first
/// if you want to preserve the current state before restoring.
pub async fn restore_snapshot(data_dir: &Path, snapshot_id: &str) -> Result<()> {
    let src = snapshots_dir(data_dir).join(snapshot_id);
    if !src.exists() {
        anyhow::bail!("snapshot not found: {}", snapshot_id);
    }

    let dest = policies_dir(data_dir);

    // Remove current policies if they exist.
    if dest.exists() {
        tokio::fs::remove_dir_all(&dest)
            .await
            .with_context(|| format!("remove current policies dir: {}", dest.display()))?;
    }

    tokio::fs::create_dir_all(&dest)
        .await
        .with_context(|| format!("recreate policies dir: {}", dest.display()))?;

    copy_dir_recursive(&src, &dest).await?;
    info!(snapshot_id = %snapshot_id, "evals: restored policy snapshot");
    Ok(())
}

// ─── List ─────────────────────────────────────────────────────────────────────

/// List available snapshot IDs, sorted newest first.
pub async fn list_snapshots(data_dir: &Path) -> Result<Vec<String>> {
    let dir = snapshots_dir(data_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = tokio::fs::read_dir(&dir)
        .await
        .with_context(|| format!("read snapshots dir: {}", dir.display()))?;

    let mut ids: Vec<String> = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                ids.push(name.to_string());
            }
        }
    }

    // Newest first — timestamp strings sort lexicographically.
    ids.sort_by(|a, b| b.cmp(a));
    Ok(ids)
}

// ─── Private helpers ──────────────────────────────────────────────────────────

/// Recursively copy a directory's contents.  Destination must already exist.
async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    let mut entries = tokio::fs::read_dir(src)
        .await
        .with_context(|| format!("read dir: {}", src.display()))?;

    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            tokio::fs::create_dir_all(&dst_path).await?;
            let s = src_path.clone();
            let d = dst_path.clone();
            // Use Box::pin for recursive async call.
            Box::pin(copy_dir_recursive(&s, &d)).await?;
        } else {
            tokio::fs::copy(&src_path, &dst_path)
                .await
                .with_context(|| {
                    format!("copy {} -> {}", src_path.display(), dst_path.display())
                })?;
        }
    }
    Ok(())
}
