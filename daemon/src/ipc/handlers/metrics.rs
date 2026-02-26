// ipc/handlers/metrics.rs — metrics.list + metrics.summary RPC handlers (Sprint PP OB.5).

use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppContext;

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// `metrics.list` — list recent metric ticks for a session.
///
/// Params:
///   `session_id`: string (required)
///   `limit`: int (optional, default 100)
pub async fn list(params: Value, ctx: Arc<AppContext>) -> Result<Value> {
    let session_id = params["session_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("session_id required"))?;
    let limit = params["limit"].as_i64().unwrap_or(100);

    let entries = ctx.metrics_store.list_session(session_id, limit).await?;
    let entries_json: Vec<Value> = entries
        .iter()
        .map(|e| {
            json!({
                "id": e.id,
                "session_id": e.session_id,
                "timestamp": e.timestamp,
                "tokens_in": e.tokens_in,
                "tokens_out": e.tokens_out,
                "tool_calls": e.tool_calls,
                "cost_usd": e.cost_usd,
            })
        })
        .collect();

    Ok(json!({ "entries": entries_json }))
}

/// `metrics.summary` — aggregate summary over a time window.
///
/// Params:
///   `since`: int (optional, default 24h ago)
///   `until`: int (optional, default now)
pub async fn summary(params: Value, ctx: Arc<AppContext>) -> Result<Value> {
    let now = now_secs();
    let since = params["since"].as_i64().unwrap_or(now - 86400);
    let until = params["until"].as_i64().unwrap_or(now);

    let s = ctx.metrics_store.summary(since, until).await?;

    Ok(json!({
        "total_tokens_in": s.total_tokens_in,
        "total_tokens_out": s.total_tokens_out,
        "total_tool_calls": s.total_tool_calls,
        "total_cost_usd": s.total_cost_usd,
        "session_count": s.session_count,
        "period_start": s.period_start,
        "period_end": s.period_end,
    }))
}

/// `metrics.rollups` — hourly rollups for graphing.
///
/// Params:
///   `since`: int (optional, default 7 days ago)
///   `until`: int (optional, default now)
pub async fn rollups(params: Value, ctx: Arc<AppContext>) -> Result<Value> {
    let now = now_secs();
    let since = params["since"].as_i64().unwrap_or(now - 86400 * 7);
    let until = params["until"].as_i64().unwrap_or(now);

    let rows = ctx.metrics_store.rollups(since, until).await?;
    let rollups_json: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "session_id": r.session_id,
                "hour_bucket": r.hour_bucket,
                "tokens_in": r.tokens_in,
                "tokens_out": r.tokens_out,
                "tool_calls": r.tool_calls,
                "cost_usd": r.cost_usd,
            })
        })
        .collect();

    Ok(json!({ "rollups": rollups_json }))
}
