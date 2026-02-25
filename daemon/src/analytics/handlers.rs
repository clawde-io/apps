// SPDX-License-Identifier: MIT
//! Analytics RPC handlers — Sprint Q (AN.T01–AN.T06).
//!
//! Dispatch entries (add to `ipc/mod.rs` dispatch match):
//!
//! ```text
//! "analytics.personal"         => analytics::handlers::personal(params, ctx).await,
//! "analytics.providerBreakdown" => analytics::handlers::provider_breakdown(params, ctx).await,
//! "analytics.session"          => analytics::handlers::session(params, ctx).await,
//! "achievements.list"          => analytics::handlers::achievements_list(params, ctx).await,
//! ```

use crate::AppContext;
use anyhow::Result;
use chrono::Utc;
use serde_json::{json, Value};

use super::storage::AnalyticsStorage;

// ─── analytics.personal ──────────────────────────────────────────────────────

/// `analytics.personal` — personal usage summary for the last N days.
///
/// Params (all optional):
/// ```json
/// { "from": "2026-01-01T00:00:00Z" }
/// ```
/// When `from` is omitted, defaults to 30 days ago.
///
/// Response:
/// ```json
/// {
///   "linesWritten":     1234,
///   "aiAssistPercent":  87.5,
///   "languages":        { "Rust": 800, "Dart": 434 },
///   "sessionsPerDay":   [ { "date": "2026-02-25", "count": 3 }, … ]
/// }
/// ```
pub async fn personal(params: Value, ctx: &AppContext) -> Result<Value> {
    let from = params
        .get("from")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    // Default: 30 days ago in UTC
    let from_str: String;
    let from = if from.is_empty() {
        let thirty_days_ago = Utc::now() - chrono::Duration::days(30);
        from_str = thirty_days_ago.to_rfc3339();
        from_str.as_str()
    } else {
        from
    };

    let storage = AnalyticsStorage::new(ctx.storage.pool());
    let analytics = storage.get_personal_analytics(from).await?;

    Ok(json!({
        "linesWritten":    analytics.lines_written,
        "aiAssistPercent": analytics.ai_assist_percent,
        "languages":       analytics.languages,
        "sessionsPerDay":  analytics.sessions_per_day,
    }))
}

// ─── analytics.providerBreakdown ─────────────────────────────────────────────

/// `analytics.providerBreakdown` — per-provider usage breakdown.
///
/// Params (all optional):
/// ```json
/// { "from": "2026-01-01T00:00:00Z" }
/// ```
///
/// Response:
/// ```json
/// [
///   { "provider": "claude", "sessions": 42, "tokens": 12345,
///     "costUsd": 0.89, "winRate": 0.71 },
///   …
/// ]
/// ```
pub async fn provider_breakdown(params: Value, ctx: &AppContext) -> Result<Value> {
    let from = params
        .get("from")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    let from_str: String;
    let from = if from.is_empty() {
        let thirty_days_ago = Utc::now() - chrono::Duration::days(30);
        from_str = thirty_days_ago.to_rfc3339();
        from_str.as_str()
    } else {
        from
    };

    let storage = AnalyticsStorage::new(ctx.storage.pool());
    let breakdown = storage.get_provider_breakdown(from).await?;

    let result: Vec<Value> = breakdown
        .into_iter()
        .map(|b| {
            json!({
                "provider": b.provider,
                "sessions": b.sessions,
                "tokens":   b.tokens,
                "costUsd":  b.cost_usd,
                "winRate":  b.win_rate,
            })
        })
        .collect();

    Ok(Value::Array(result))
}

// ─── analytics.session ───────────────────────────────────────────────────────

/// `analytics.session` — per-session analytics.
///
/// Params:
/// ```json
/// { "sessionId": "abc123" }
/// ```
///
/// Response:
/// ```json
/// {
///   "sessionId":    "abc123",
///   "durationSecs": 1234,
///   "messageCount": 18,
///   "provider":     "claude",
///   "linesWritten": 0
/// }
/// ```
pub async fn session(params: Value, ctx: &AppContext) -> Result<Value> {
    let session_id = params
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("sessionId required"))?;

    let storage = AnalyticsStorage::new(ctx.storage.pool());
    let sa = storage.get_session_analytics(session_id).await?;

    Ok(json!({
        "sessionId":    sa.session_id,
        "durationSecs": sa.duration_secs,
        "messageCount": sa.message_count,
        "provider":     sa.provider,
        "linesWritten": sa.lines_written,
    }))
}

// ─── achievements.list ────────────────────────────────────────────────────────

/// `achievements.list` — list all achievements with unlock status.
///
/// Params: `{}` (none required)
///
/// Response:
/// ```json
/// [
///   {
///     "id":          "first_session",
///     "name":        "First Session",
///     "description": "Started your first AI session. The journey begins.",
///     "unlocked":    true,
///     "unlockedAt":  "2026-02-25T10:30:00Z"
///   },
///   …
/// ]
/// ```
pub async fn achievements_list(_params: Value, ctx: &AppContext) -> Result<Value> {
    let storage = AnalyticsStorage::new(ctx.storage.pool());
    let achievements = storage.list_achievements().await?;

    let result: Vec<Value> = achievements
        .into_iter()
        .map(|a| {
            json!({
                "id":          a.id,
                "name":        a.name,
                "description": a.description,
                "unlocked":    a.unlocked,
                "unlockedAt":  a.unlocked_at,
            })
        })
        .collect();

    Ok(Value::Array(result))
}
