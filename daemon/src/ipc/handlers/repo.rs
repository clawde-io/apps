use crate::{security, AppContext};
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// Max file size returned by `repo.readFile` (1 MiB). Larger files are rejected.
const MAX_READ_FILE_BYTES: u64 = 1_048_576;

#[derive(Deserialize)]
struct RepoPathParams {
    #[serde(rename = "repoPath")]
    repo_path: String,
}

#[derive(Deserialize)]
struct FileDiffParams {
    #[serde(rename = "repoPath")]
    repo_path: String,
    path: String,
    staged: Option<bool>,
}

#[derive(Deserialize)]
struct ReadFileParams {
    #[serde(rename = "repoPath")]
    repo_path: String,
    path: String,
}

/// Reject relative paths and null bytes — prevents directory traversal attacks.
fn validate_repo_path(path: &str) -> Result<()> {
    if path.contains('\0') {
        bail!("invalid repoPath: null byte");
    }
    if !Path::new(path).is_absolute() {
        bail!("invalid repoPath: must be an absolute path");
    }
    Ok(())
}

/// Reject paths that escape the repo root via `..` components or are absolute.
fn validate_file_path(path: &str) -> Result<()> {
    if path.contains('\0') {
        bail!("invalid path: null byte");
    }
    if Path::new(path).is_absolute() {
        bail!("invalid path: must be relative to the repository root");
    }
    // Normalise and check for traversal components.
    let p = std::path::PathBuf::from(path);
    for component in p.components() {
        if component == std::path::Component::ParentDir {
            bail!("invalid path: directory traversal not allowed");
        }
    }
    Ok(())
}

pub async fn open(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;
    // DC.T41: check for data-dir overlap and .clawd/ injection
    security::check_repo_path_safety(Path::new(&p.repo_path), &ctx.config.data_dir)?;
    let status = ctx.repo_registry.open(&p.repo_path).await?;
    Ok(serde_json::to_value(status)?)
}

pub async fn status(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;
    let status = ctx.repo_registry.status(&p.repo_path).await?;
    Ok(serde_json::to_value(status)?)
}

pub async fn close(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;
    let removed = ctx.repo_registry.close(&p.repo_path).await?;
    if !removed {
        bail!(
            "REPO_NOT_FOUND: repo is not currently tracked: {}",
            p.repo_path
        );
    }
    Ok(json!({ "closed": true }))
}

pub async fn diff(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;
    let diffs = ctx.repo_registry.diff(&p.repo_path).await?;
    Ok(json!(diffs))
}

pub async fn file_diff(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: FileDiffParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;
    validate_file_path(&p.path)?;
    let diff = ctx
        .repo_registry
        .file_diff(&p.repo_path, &p.path, p.staged.unwrap_or(false))
        .await?;
    Ok(serde_json::to_value(diff)?)
}

/// `repo.list` — returns all currently registered repos with path, branch, and status.
pub async fn list(_params: Value, ctx: &AppContext) -> Result<Value> {
    let repos = ctx.repo_registry.list().await;
    Ok(serde_json::Value::Array(repos))
}

/// `repo.tree` — returns a flat sorted list of all tracked file paths in a repo.
pub async fn tree(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;
    let files = ctx.repo_registry.list_files(&p.repo_path).await?;
    Ok(json!({ "repoPath": p.repo_path, "files": files }))
}

/// `repo.readFile` — reads a tracked file's content from disk.
///
/// Rejects files > 1 MiB and any path that resolves outside the repo root.
pub async fn read_file(params: Value, _ctx: &AppContext) -> Result<Value> {
    let p: ReadFileParams = serde_json::from_value(params)?;
    validate_repo_path(&p.repo_path)?;
    validate_file_path(&p.path)?;

    let repo_root = std::path::PathBuf::from(&p.repo_path);
    let joined = repo_root.join(&p.path);

    // Canonicalize to catch symlink-based traversal.
    let canonical = tokio::fs::canonicalize(&joined)
        .await
        .map_err(|e| anyhow::anyhow!("cannot resolve path '{}': {}", p.path, e))?;
    let canonical_root = tokio::fs::canonicalize(&repo_root)
        .await
        .map_err(|e| anyhow::anyhow!("cannot resolve repo root '{}': {}", p.repo_path, e))?;
    if !canonical.starts_with(&canonical_root) {
        bail!("path escapes repository root");
    }

    // Guard file size before reading.
    let meta = tokio::fs::metadata(&canonical)
        .await
        .map_err(|e| anyhow::anyhow!("cannot stat '{}': {}", p.path, e))?;
    if meta.len() > MAX_READ_FILE_BYTES {
        bail!(
            "file too large: {} bytes (max {} bytes)",
            meta.len(),
            MAX_READ_FILE_BYTES
        );
    }

    let content = tokio::fs::read_to_string(&canonical)
        .await
        .map_err(|e| anyhow::anyhow!("cannot read '{}': {}", p.path, e))?;

    Ok(json!({
        "repoPath": p.repo_path,
        "path": p.path,
        "content": content,
        "sizeBytes": meta.len(),
    }))
}
