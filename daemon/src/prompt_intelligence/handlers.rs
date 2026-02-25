// SPDX-License-Identifier: MIT
//! RPC handlers for the prompt intelligence subsystem (Sprint W).
//!
//! Registered methods:
//!   prompt.suggest     — return heuristic suggestions for the current input
//!   prompt.recordUsed  — increment use count for a prompt in history

use super::suggester::PromptSuggester;
use crate::repo_intelligence;
use crate::AppContext;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

// ─── Param structs ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SuggestParams {
    /// The text the user has typed so far (may be empty).
    #[serde(default)]
    current_input: String,
    /// Serialised session context (recent messages summary, optional).
    #[serde(default)]
    session_context: String,
    /// Absolute repo path for profile-aware suggestions (optional).
    #[serde(default)]
    repo_path: Option<String>,
    /// Maximum number of suggestions to return (default: 5).
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordUsedParams {
    /// The exact prompt text that was submitted.
    prompt: String,
    /// The session the prompt was submitted in.
    session_id: String,
}

fn default_limit() -> usize {
    5
}

// ─── prompt.suggest ──────────────────────────────────────────────────────────

/// `prompt.suggest` — return heuristic prompt suggestions.
pub async fn prompt_suggest(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SuggestParams = serde_json::from_value(params)?;
    let pool = ctx.storage.pool();

    // Load repo profile if a path was supplied.
    let profile = if let Some(ref rp) = p.repo_path {
        if !rp.contains('\0') && std::path::Path::new(rp.as_str()).is_absolute() {
            repo_intelligence::storage::load(&pool, rp).await.unwrap_or(None)
        } else {
            None
        }
    } else {
        None
    };

    let suggestions = PromptSuggester::suggest_prompts(
        &pool,
        &p.current_input,
        &p.session_context,
        &profile,
        p.limit,
    )
    .await?;

    Ok(json!({ "suggestions": suggestions }))
}

// ─── prompt.recordUsed ───────────────────────────────────────────────────────

/// `prompt.recordUsed` — record that a prompt was used.
pub async fn prompt_record_used(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RecordUsedParams = serde_json::from_value(params)?;
    if p.prompt.is_empty() {
        anyhow::bail!("prompt must not be empty");
    }
    let pool = ctx.storage.pool();
    PromptSuggester::record_prompt_used(&pool, &p.prompt, &p.session_id).await?;
    Ok(json!({ "recorded": true }))
}
