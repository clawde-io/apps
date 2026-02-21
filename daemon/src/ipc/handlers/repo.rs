use crate::AppContext;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

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

pub async fn open(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    let status = ctx.repo_registry.open(&p.repo_path).await?;
    Ok(serde_json::to_value(status)?)
}

pub async fn status(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    let status = ctx.repo_registry.status(&p.repo_path).await?;
    Ok(serde_json::to_value(status)?)
}

pub async fn diff(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RepoPathParams = serde_json::from_value(params)?;
    let diffs = ctx.repo_registry.diff(&p.repo_path).await?;
    Ok(json!(diffs))
}

pub async fn file_diff(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: FileDiffParams = serde_json::from_value(params)?;
    let diff = ctx
        .repo_registry
        .file_diff(&p.repo_path, &p.path, p.staged.unwrap_or(false))
        .await?;
    Ok(serde_json::to_value(diff)?)
}
