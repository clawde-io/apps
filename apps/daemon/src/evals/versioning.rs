//! Rule versioning — SHA-256 hashing of policy file contents.
//!
//! A policy "version" is the hex-encoded SHA-256 digest of all policy files
//! in `.claw/policies/`, sorted by filename for determinism.  Saving and
//! loading the version lets the daemon detect when policies change between
//! daemon restarts.

use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use tracing::debug;

// ─── Hashing ─────────────────────────────────────────────────────────────────

/// Compute a single SHA-256 digest that represents ALL policy files in the
/// given directory.  Files are processed in sorted order for determinism.
///
/// Returns an empty string if the directory does not exist or contains no
/// readable files — this is not an error condition.
pub async fn hash_policy_dir(policies_dir: &Path) -> Result<String> {
    if !policies_dir.exists() {
        return Ok(String::new());
    }

    // Collect all file paths under the policy directory (not recursive — one level).
    let mut entries = tokio::fs::read_dir(policies_dir)
        .await
        .with_context(|| format!("read policies dir: {}", policies_dir.display()))?;

    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let p = entry.path();
        if p.is_file() {
            paths.push(p);
        }
    }

    if paths.is_empty() {
        return Ok(String::new());
    }

    // Sort for deterministic ordering.
    paths.sort();

    let mut hasher = Sha256::new();
    for path in &paths {
        let contents = tokio::fs::read(path)
            .await
            .with_context(|| format!("read policy file: {}", path.display()))?;
        // Hash the filename too so renames are detected.
        if let Some(name) = path.file_name() {
            hasher.update(name.to_string_lossy().as_bytes());
            hasher.update(b"\0");
        }
        hasher.update(&contents);
        hasher.update(b"\0");
    }

    let digest = hasher.finalize();
    let hex = format!("{:x}", digest);
    debug!(hash = %hex, files = paths.len(), "policy dir hashed");
    Ok(hex)
}

// ─── Persistence ──────────────────────────────────────────────────────────────

const VERSION_FILE: &str = "evals/rule-version.json";

/// Load the previously saved rule version from `.claw/evals/rule-version.json`.
///
/// Returns `None` if the file does not exist (first run).
pub async fn load_rule_version(data_dir: &Path) -> Result<Option<String>> {
    let path = data_dir.join(VERSION_FILE);
    match tokio::fs::read_to_string(&path).await {
        Ok(s) => {
            let v: serde_json::Value =
                serde_json::from_str(&s).with_context(|| "parse rule-version.json")?;
            Ok(v.get("hash").and_then(|h| h.as_str()).map(String::from))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).context("read rule-version.json"),
    }
}

/// Persist the current rule version to `.claw/evals/rule-version.json`.
pub async fn save_rule_version(data_dir: &Path, hash: &str) -> Result<()> {
    let evals_dir = data_dir.join("evals");
    tokio::fs::create_dir_all(&evals_dir)
        .await
        .with_context(|| format!("create evals dir: {}", evals_dir.display()))?;

    let path = data_dir.join(VERSION_FILE);
    let payload = serde_json::json!({
        "hash": hash,
        "updated_at": chrono::Utc::now().to_rfc3339(),
    });
    let content = serde_json::to_string_pretty(&payload)?;
    tokio::fs::write(&path, content.as_bytes())
        .await
        .with_context(|| format!("write rule-version.json: {}", path.display()))?;
    Ok(())
}
