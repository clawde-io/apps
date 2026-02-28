// SPDX-License-Identifier: MIT
//! RPC handlers for the dead-letter queue.
//!
//! Methods:
//!   - `dead_letter.list`  — list failed events (optional status filter + limit)
//!   - `dead_letter.retry` — reset a specific event to `pending` for re-delivery

use anyhow::Result;
use serde_json::{json, Value};

use crate::{events::dead_letter, AppContext};

/// `dead_letter.list` — list dead-letter events.
///
/// Params: `{ status?: "pending"|"permanently_failed", limit?: number }`
///
/// Returns: `{ events: [...] }`
pub async fn list(params: Value, ctx: &AppContext) -> Result<Value> {
    let status = params.get("status").and_then(|v| v.as_str());
    let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(50);
    let limit = limit.clamp(1, 200);

    let events = dead_letter::list_dead_letter(&ctx.storage, status, limit).await?;

    let events_json: Vec<Value> = events
        .iter()
        .map(|e| {
            json!({
                "id": e.id,
                "sourceSessionId": e.source_session_id,
                "eventType": e.event_type,
                "payload": e.payload_value(),
                "failureReason": e.failure_reason,
                "retryCount": e.retry_count,
                "status": e.status,
                "createdAt": e.created_at,
                "lastAttemptedAt": e.last_attempted_at,
            })
        })
        .collect();

    Ok(json!({ "events": events_json }))
}

/// `dead_letter.retry` — manually reset a dead-letter event to `pending`.
///
/// Params: `{ id: string }`
///
/// Returns: `{ found: boolean }`
pub async fn retry(params: Value, ctx: &AppContext) -> Result<Value> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required param: id"))?;

    let found = dead_letter::mark_for_retry(&ctx.storage, id).await?;
    Ok(json!({ "found": found }))
}
