//! Sprint DD TS.3/TS.7 — `sovereignty.*` RPC handlers.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// `sovereignty.report` — 7-day summary of other AI tools touching the project.
pub async fn report(_params: Value, ctx: AppContext) -> Result<Value> {
    let summaries = crate::sovereignty::tracker::get_report(ctx.storage.pool()).await?;

    Ok(json!({
        "period": "7d",
        "tools": summaries,
        "totalEvents": summaries.iter().map(|s| s.event_count).sum::<i64>(),
    }))
}

/// `sovereignty.events` — raw event list for a tool in the last N days.
pub async fn events(params: Value, ctx: AppContext) -> Result<Value> {
    use sqlx::Row as _;

    let tool_id = params.get("toolId").and_then(|v| v.as_str());
    let days = params
        .get("days")
        .and_then(|v| v.as_i64())
        .unwrap_or(7);

    let rows = if let Some(tool) = tool_id {
        sqlx::query(
            "SELECT id, tool_id, event_type, file_paths, detected_at
             FROM sovereignty_events
             WHERE tool_id = ? AND detected_at >= datetime('now', ? || ' days')
             ORDER BY detected_at DESC LIMIT 100",
        )
        .bind(tool)
        .bind(format!("-{}", days))
        .fetch_all(ctx.storage.pool())
        .await?
    } else {
        sqlx::query(
            "SELECT id, tool_id, event_type, file_paths, detected_at
             FROM sovereignty_events
             WHERE detected_at >= datetime('now', ? || ' days')
             ORDER BY detected_at DESC LIMIT 100",
        )
        .bind(format!("-{}", days))
        .fetch_all(ctx.storage.pool())
        .await?
    };

    let events: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "toolId": r.get::<String, _>("tool_id"),
                "eventType": r.get::<String, _>("event_type"),
                "filePaths": serde_json::from_str::<Value>(r.get::<String, _>("file_paths").as_str()).unwrap_or(json!([])),
                "detectedAt": r.get::<String, _>("detected_at"),
            })
        })
        .collect();

    Ok(json!({ "events": events, "count": events.len() }))
}
