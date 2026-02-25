// SPDX-License-Identifier: MIT
//! RPC handlers for the AI Code Review Engine — Sprint O
//!
//! Exposed methods:
//! - `review.run`    — run a full code review for a session's repo
//! - `review.fix`    — apply auto-fixes from a previous review
//! - `review.learn`  — record user feedback on a review comment

use crate::code_review::model::ReviewConfig;
use crate::code_review::workflow;
use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

/// `review.run` — run a code review.
///
/// Params:
/// - `repo_path`: string — absolute path to the repository root
/// - `config`: optional ReviewConfig JSON object
pub async fn run(params: Value, _ctx: &AppContext) -> Result<Value> {
    let repo_path = params
        .get("repo_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("repo_path required"))?;

    let config: ReviewConfig = params
        .get("config")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let result = workflow::run_review(&PathBuf::from(repo_path), &config).await?;
    Ok(serde_json::to_value(result)?)
}

/// `review.fix` — apply auto-fixes from a review result.
///
/// Params:
/// - `review_id`: string — the review UUID returned by `review.run`
/// - `issue_codes`: optional array of rule codes to fix (empty = fix all)
pub async fn fix(params: Value, _ctx: &AppContext) -> Result<Value> {
    let review_id = params
        .get("review_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("review_id required"))?;

    // Future: look up stored review, apply --fix for each fixable issue.
    tracing::info!("review.fix called for review_id={}", review_id);
    Ok(json!({ "status": "ok", "fixed": 0, "review_id": review_id }))
}

/// `review.learn` — record feedback on a review comment.
///
/// Params:
/// - `review_id`: string
/// - `comment_index`: integer — index into the comments array
/// - `useful`: boolean — whether the comment was helpful
/// - `note`: optional string — user's note
pub async fn learn(params: Value, _ctx: &AppContext) -> Result<Value> {
    let review_id = params
        .get("review_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("review_id required"))?;
    let useful = params
        .get("useful")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    tracing::info!("review.learn: review_id={} useful={}", review_id, useful);
    Ok(json!({ "status": "ok" }))
}
