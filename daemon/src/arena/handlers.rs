// SPDX-License-Identifier: MIT
// Arena Mode — JSON-RPC 2.0 handlers (Sprint K, AM.T01–AM.T03).
//
// arena.createSession  — spawn two parallel sessions on the same prompt.
// arena.vote           — record which provider the user preferred.
// arena.leaderboard    — return win-rate rankings, optionally filtered by task type.

use crate::AppContext;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;

use super::storage::ArenaStorage;

// ─── Valid values ─────────────────────────────────────────────────────────────

const VALID_PROVIDERS: &[&str] = &["claude", "codex", "cursor"];
const VALID_TASK_TYPES: &[&str] = &["general", "debug", "refactor", "explain", "generate"];

// ─── arena.createSession ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateArenaParams {
    /// Repo path shared by both sessions.
    #[serde(rename = "repoPath")]
    repo_path: String,
    /// Provider for session A (e.g. "claude").
    #[serde(rename = "providerA")]
    provider_a: String,
    /// Provider for session B (e.g. "codex").
    #[serde(rename = "providerB")]
    provider_b: String,
    /// The prompt to send to both sessions simultaneously.
    prompt: String,
}

/// `arena.createSession` — create a blind arena comparison.
///
/// Spawns two sessions simultaneously (provider A and provider B) on the same
/// prompt.  Returns the arena session ID together with both session IDs so the
/// client can subscribe to their message streams.
pub async fn create_arena_session(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: CreateArenaParams = serde_json::from_value(params)?;

    // Validate providers.
    if !VALID_PROVIDERS.contains(&p.provider_a.as_str()) {
        anyhow::bail!(
            "invalid type: unknown providerA '{}' — must be one of: {}",
            p.provider_a,
            VALID_PROVIDERS.join(", ")
        );
    }
    if !VALID_PROVIDERS.contains(&p.provider_b.as_str()) {
        anyhow::bail!(
            "invalid type: unknown providerB '{}' — must be one of: {}",
            p.provider_b,
            VALID_PROVIDERS.join(", ")
        );
    }
    if p.provider_a == p.provider_b {
        anyhow::bail!("invalid type: providerA and providerB must be different");
    }
    if p.prompt.trim().is_empty() {
        anyhow::bail!("invalid type: prompt must not be empty");
    }
    if !std::path::Path::new(&p.repo_path).exists() {
        anyhow::bail!("REPO_NOT_FOUND: repo path does not exist: {}", p.repo_path);
    }

    // Spawn the two sessions concurrently.  The initial message is sent to
    // each session immediately so both providers start generating in parallel.
    let title_a = format!("Arena — {}", p.provider_a);
    let title_b = format!("Arena — {}", p.provider_b);

    let (session_a_res, session_b_res) = tokio::join!(
        ctx.session_manager.create(
            &p.provider_a,
            &p.repo_path,
            &title_a,
            ctx.config.max_sessions,
            None,
            Some(&p.prompt),
        ),
        ctx.session_manager.create(
            &p.provider_b,
            &p.repo_path,
            &title_b,
            ctx.config.max_sessions,
            None,
            Some(&p.prompt),
        ),
    );

    let session_a = session_a_res?;
    let session_b = session_b_res?;

    // Send the prompt to both sessions so they start generating responses.
    let _ = tokio::join!(
        ctx.session_manager
            .send_message(&session_a.id, &p.prompt, ctx),
        ctx.session_manager
            .send_message(&session_b.id, &p.prompt, ctx),
    );

    // Persist the arena session record.
    let arena_storage = ArenaStorage::new(ctx.storage.pool());
    let arena = arena_storage
        .create_session(
            &session_a.id,
            &session_b.id,
            &p.provider_a,
            &p.provider_b,
            &p.prompt,
        )
        .await?;

    info!(
        arena_id = %arena.id,
        provider_a = %p.provider_a,
        provider_b = %p.provider_b,
        "arena session created"
    );

    Ok(json!({
        "arenaSessionId": arena.id,
        "sessionAId": session_a.id,
        "sessionBId": session_b.id,
        "providerA": arena.provider_a,
        "providerB": arena.provider_b,
    }))
}

// ─── arena.vote ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct VoteParams {
    /// Arena session ID.
    #[serde(rename = "arenaSessionId")]
    arena_session_id: String,
    /// Provider the user preferred ("claude", "codex", or "cursor").
    #[serde(rename = "winnerProvider")]
    winner_provider: String,
    /// Optional task category for leaderboard segmentation.
    /// Defaults to "general" if omitted.
    #[serde(rename = "taskType")]
    task_type: Option<String>,
}

/// `arena.vote` — record which provider the user preferred.
///
/// Stores the vote in `arena_votes` and broadcasts an `arena.voted` event.
pub async fn record_vote(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: VoteParams = serde_json::from_value(params)?;

    // Validate winner provider.
    if !VALID_PROVIDERS.contains(&p.winner_provider.as_str()) {
        anyhow::bail!(
            "invalid type: unknown winnerProvider '{}' — must be one of: {}",
            p.winner_provider,
            VALID_PROVIDERS.join(", ")
        );
    }

    let task_type = p.task_type.unwrap_or_else(|| "general".to_string());
    if !VALID_TASK_TYPES.contains(&task_type.as_str()) {
        anyhow::bail!(
            "invalid type: unknown taskType '{}' — must be one of: {}",
            task_type,
            VALID_TASK_TYPES.join(", ")
        );
    }

    let arena_storage = ArenaStorage::new(ctx.storage.pool());
    let vote = arena_storage
        .record_vote(&p.arena_session_id, &p.winner_provider, &task_type)
        .await?;

    // Broadcast so the Flutter UI can reveal provider labels immediately.
    ctx.broadcaster.broadcast(
        "arena.voted",
        json!({
            "arenaSessionId": p.arena_session_id,
            "winnerProvider": p.winner_provider,
            "taskType": task_type,
        }),
    );

    // Check if we've crossed the auto-routing threshold (AM.T04).
    let vote_count = arena_storage.get_vote_count().await?;
    if vote_count == 20 {
        ctx.broadcaster
            .broadcast("arena.autoRouteEnabled", json!({ "voteCount": vote_count }));
    }

    info!(
        arena_id = %p.arena_session_id,
        winner = %p.winner_provider,
        task_type = %task_type,
        "arena vote recorded"
    );

    Ok(json!({ "ok": true, "voteId": vote.id }))
}

// ─── arena.leaderboard ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LeaderboardParams {
    /// Optional task type filter.  `null` returns all categories.
    #[serde(rename = "taskType")]
    task_type: Option<String>,
}

/// `arena.leaderboard` — return win-rate rankings.
///
/// Results are grouped by provider and task type.  Pass `taskType` to
/// restrict results to a single category.  When omitted, results include
/// per-task-type rows plus an aggregate "all" row per provider.
pub async fn get_leaderboard(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: LeaderboardParams =
        serde_json::from_value(params).unwrap_or(LeaderboardParams { task_type: None });

    if let Some(ref tt) = p.task_type {
        if !VALID_TASK_TYPES.contains(&tt.as_str()) {
            anyhow::bail!(
                "invalid type: unknown taskType '{}' — must be one of: {}",
                tt,
                VALID_TASK_TYPES.join(", ")
            );
        }
    }

    let arena_storage = ArenaStorage::new(ctx.storage.pool());
    let entries = arena_storage
        .get_leaderboard(p.task_type.as_deref())
        .await?;
    let vote_count = arena_storage.get_vote_count().await?;

    Ok(json!({
        "entries": entries,
        "totalVotes": vote_count,
        "autoRouteEnabled": vote_count >= 20,
    }))
}
