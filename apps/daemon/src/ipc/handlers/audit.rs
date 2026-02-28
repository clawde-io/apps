/// `session.toolCallAudit` — query the tool call audit log (DC.T43).
///
/// Params: `{ "sessionId"?: string, "limit"?: number, "before"?: string }`
/// Returns: `{ "events": [...], "count": N }`
use crate::AppContext;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct AuditParams {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    limit: Option<i64>,
    before: Option<String>,
}

pub async fn tool_call_audit(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: AuditParams = serde_json::from_value(params)?;
    let limit = p.limit.unwrap_or(50).min(200);

    let events = ctx
        .storage
        .list_tool_call_events(p.session_id.as_deref(), limit, p.before.as_deref())
        .await?;

    let count = events.len();
    Ok(json!({ "events": events, "count": count }))
}
