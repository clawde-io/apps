use crate::AppContext;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

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
        bail!("REPO_NOT_FOUND: repo is not currently tracked: {}", p.repo_path);
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
