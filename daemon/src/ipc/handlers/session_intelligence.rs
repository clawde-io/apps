// SPDX-License-Identifier: MIT
//! RPC handlers for the Session Intelligence subsystem (Sprint G, SI.T02–T12).
//!
//! Registered methods:
//!   message.pin           — pin a message so it always stays in context (SI.T04)
//!   message.unpin         — unpin a message (SI.T04)
//!   session.contextStatus — context window usage vs model limit (SI.T02)
//!   session.health        — session response quality health score (SI.T06)
//!   session.splitProposed — complexity analysis + split proposal (SI.T10)
//!   context.bridge        — build bridge context for a new session (SI.T08)

use crate::session_intelligence::{
    bridge, complexity,
    context_guard::{check_context_health, ModelLimit},
    health,
};
use crate::AppContext;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::FromRow;

// ─── Param structs ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct MessageIdParams {
    #[serde(rename = "messageId")]
    message_id: String,
}

#[derive(Deserialize)]
struct SessionIdParams {
    #[serde(rename = "sessionId")]
    session_id: String,
}

#[derive(Deserialize)]
struct ContextStatusParams {
    #[serde(rename = "sessionId")]
    session_id: String,
    /// Provider name (e.g. "claude", "cursor") to determine the context limit.
    #[serde(default)]
    provider: String,
}

#[derive(Deserialize)]
struct SplitProposedParams {
    /// The user's prompt to analyse for complexity.
    prompt: String,
}

// ─── message.pin ──────────────────────────────────────────────────────────────

pub async fn pin_message(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: MessageIdParams = serde_json::from_value(params)?;
    ctx.storage.pin_message(&p.message_id).await?;
    Ok(json!({ "pinned": true }))
}

// ─── message.unpin ────────────────────────────────────────────────────────────

pub async fn unpin_message(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: MessageIdParams = serde_json::from_value(params)?;
    ctx.storage.unpin_message(&p.message_id).await?;
    Ok(json!({ "pinned": false }))
}

// ─── session.contextStatus ───────────────────────────────────────────────────

/// Returns the context window utilisation for a session.
///
/// Sums the stored `token_count` of all messages and compares against the
/// model limit derived from the provider name.
pub async fn context_status(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: ContextStatusParams = serde_json::from_value(params)?;

    let pool = ctx.storage.pool();

    // Sum token counts stored in the messages table.
    #[derive(FromRow)]
    struct TokenSumRow {
        total: i64,
    }
    let row: TokenSumRow = sqlx::query_as(
        "SELECT COALESCE(SUM(token_count), 0) AS total FROM messages WHERE session_id = ?",
    )
    .bind(&p.session_id)
    .fetch_one(&pool)
    .await?;

    let total_tokens = row.total as usize;
    let limit = ModelLimit::from_provider(&p.provider);
    let status = check_context_health(total_tokens, limit);

    Ok(json!({
        "sessionId":   p.session_id,
        "usedTokens":  total_tokens,
        "maxTokens":   limit.max_tokens(),
        "percent":     status.percent(),
        "status":      match &status {
            crate::session_intelligence::context_guard::ContextStatus::Ok    { .. } => "ok",
            crate::session_intelligence::context_guard::ContextStatus::Warning { .. } => "warning",
            crate::session_intelligence::context_guard::ContextStatus::Critical { .. } => "critical",
        },
    }))
}

// ─── session.health ───────────────────────────────────────────────────────────

pub async fn session_health(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SessionIdParams = serde_json::from_value(params)?;

    let state = health::load_or_create(&ctx.storage, &p.session_id).await?;

    Ok(json!({
        "sessionId":             state.session_id,
        "healthScore":           state.health_score,
        "totalTurns":            state.total_turns,
        "consecutiveLowQuality": state.consecutive_low_quality,
        "shortResponseCount":    state.short_response_count,
        "toolErrorCount":        state.tool_error_count,
        "truncationCount":       state.truncation_count,
        "needsRefresh":          state.needs_refresh(),
    }))
}

// ─── session.splitProposed ───────────────────────────────────────────────────

pub async fn split_proposed(params: Value, _ctx: &AppContext) -> Result<Value> {
    let p: SplitProposedParams = serde_json::from_value(params)?;

    if p.prompt.trim().is_empty() {
        bail!("INVALID_PARAMS: prompt must not be empty");
    }

    let complexity = complexity::classify_prompt(&p.prompt);
    let proposal = complexity::build_split_proposal(&p.prompt);

    let proposal_json = proposal.map(|prop| {
        json!({
            "complexity": prop.complexity,
            "subtasks":   prop.subtasks,
            "reason":     prop.reason,
        })
    });

    Ok(json!({
        "complexity":  complexity,
        "shouldSplit": complexity.should_split(),
        "proposal":    proposal_json,
    }))
}

// ─── context.bridge ──────────────────────────────────────────────────────────

pub async fn context_bridge(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: SessionIdParams = serde_json::from_value(params)?;

    let bridge_ctx = bridge::build_bridge(&ctx.storage, &p.session_id).await?;
    let injection_text = bridge_ctx.to_injection_text();

    Ok(json!({
        "sourceSessionId":      bridge_ctx.source_session_id,
        "systemPrompt":         bridge_ctx.system_prompt,
        "pinnedMessages":       bridge_ctx.pinned_messages,
        "lastUserMessage":      bridge_ctx.last_user_message,
        "lastAssistantMessage": bridge_ctx.last_assistant_message,
        "sourceTurnCount":      bridge_ctx.source_turn_count,
        "repoPath":             bridge_ctx.repo_path,
        "injectionText":        injection_text,
    }))
}
