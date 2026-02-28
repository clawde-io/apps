//! Sprint DD PP.3 — `project.pulse` RPC handler.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// `project.pulse` — semantic change velocity for the project.
///
/// Returns a structured pulse report for the last N days.
pub async fn pulse(params: Value, ctx: &AppContext) -> Result<Value> {
    let days = params.get("days").and_then(|v| v.as_i64()).unwrap_or(7);

    let events = crate::analysis::semantic_delta::get_pulse(ctx.storage.pool(), days).await?;

    // Compute velocity breakdown.
    let mut features = 0i64;
    let mut bugs = 0i64;
    let mut refactors = 0i64;
    let mut tests = 0i64;
    let mut configs = 0i64;
    let mut deps = 0i64;

    use crate::analysis::semantic_delta::SemanticEventType;
    for event in &events {
        match event.event_type {
            SemanticEventType::FeatureAdded => features += 1,
            SemanticEventType::BugFixed => bugs += 1,
            SemanticEventType::Refactored => refactors += 1,
            SemanticEventType::TestAdded => tests += 1,
            SemanticEventType::ConfigChanged => configs += 1,
            SemanticEventType::DependencyUpdated => deps += 1,
        }
    }

    let events_json: Vec<Value> = events
        .iter()
        .map(|e| {
            json!({
                "id": e.id,
                "sessionId": e.session_id,
                "taskId": e.task_id,
                "eventType": e.event_type.as_str(),
                "affectedFiles": e.affected_files,
                "summaryText": e.summary_text,
                "createdAt": e.created_at,
            })
        })
        .collect();

    Ok(json!({
        "period": format!("{}d", days),
        "events": events_json,
        "velocity": {
            "features": features,
            "bugs": bugs,
            "refactors": refactors,
            "tests": tests,
            "configs": configs,
            "deps": deps,
            "total": events.len(),
        },
    }))
}
