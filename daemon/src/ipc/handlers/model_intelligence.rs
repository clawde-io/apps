// SPDX-License-Identifier: MIT
//! RPC handlers for Model Intelligence (Sprint H, MI.T11–T12).
//!
//! Registered methods:
//!   session.setModel       — pin or clear the model override for a session (MI.T12)
//!   session.addRepoContext — add a path to the session's repo-context registry (MI.T11)
//!   session.listRepoContexts — list registered context paths for a session (MI.T11)
//!   session.removeRepoContext — remove a context entry by ID (MI.T11)

use crate::AppContext;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

// ─── Param structs ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SetModelParams {
    #[serde(rename = "sessionId")]
    session_id: String,
    /// Model ID to pin (e.g. `"claude-sonnet-4-6"`), or `null` to restore auto-routing.
    model: Option<String>,
}

#[derive(Deserialize)]
struct AddRepoContextParams {
    #[serde(rename = "sessionId")]
    session_id: String,
    /// Absolute path to the file or directory.
    path: String,
    /// Priority 1 (lowest) … 10 (highest).  Defaults to 5.
    #[serde(default = "default_priority")]
    priority: i64,
}

fn default_priority() -> i64 { 5 }

#[derive(Deserialize)]
struct SessionIdParams {
    #[serde(rename = "sessionId")]
    session_id: String,
}

#[derive(Deserialize)]
struct ContextIdParams {
    /// The `id` field of the `session_contexts` row to remove.
    id: String,
}

// ─── session.setModel ─────────────────────────────────────────────────────────

/// Pin a specific model to a session, bypassing the auto-router.
/// Pass `model: null` to restore auto-routing.
pub async fn set_model(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SetModelParams = serde_json::from_value(params)?;

    let session = ctx
        .storage
        .get_session(&p.session_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("SESSION_NOT_FOUND"))?;

    ctx.storage
        .set_model_override(&session.id, p.model.as_deref())
        .await?;

    Ok(json!({
        "sessionId":     session.id,
        "modelOverride": p.model,
    }))
}

// ─── session.addRepoContext ───────────────────────────────────────────────────

/// Register a file or directory as part of the session's repo context.
/// Duplicate paths update the priority instead of inserting a second row.
pub async fn add_repo_context(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: AddRepoContextParams = serde_json::from_value(params)?;

    if p.path.trim().is_empty() {
        bail!("INVALID_PARAMS: path must not be empty");
    }
    if !(1..=10).contains(&p.priority) {
        bail!("INVALID_PARAMS: priority must be 1–10");
    }

    let row = ctx
        .storage
        .add_repo_context(&p.session_id, &p.path, p.priority)
        .await?;

    Ok(json!({
        "id":        row.id,
        "sessionId": row.session_id,
        "path":      row.path,
        "priority":  row.priority,
        "addedAt":   row.added_at,
    }))
}

// ─── session.listRepoContexts ─────────────────────────────────────────────────

/// List all context entries for a session, highest-priority first.
pub async fn list_repo_contexts(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SessionIdParams = serde_json::from_value(params)?;

    let rows = ctx.storage.list_repo_contexts(&p.session_id).await?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id":        r.id,
                "sessionId": r.session_id,
                "path":      r.path,
                "priority":  r.priority,
                "addedAt":   r.added_at,
            })
        })
        .collect();

    Ok(json!({ "contexts": items }))
}

// ─── session.removeRepoContext ────────────────────────────────────────────────

/// Remove a context entry by its ID.
pub async fn remove_repo_context(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: ContextIdParams = serde_json::from_value(params)?;
    ctx.storage.remove_repo_context(&p.id).await?;
    Ok(json!({ "removed": true }))
}
