use crate::AppContext;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct ToolDecisionParams {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(rename = "toolCallId")]
    tool_call_id: String,
}

pub async fn approve(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: ToolDecisionParams = serde_json::from_value(params)?;
    ctx.session_manager
        .approve_tool(&p.session_id, &p.tool_call_id)
        .await?;
    Ok(json!({}))
}

pub async fn reject(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: ToolDecisionParams = serde_json::from_value(params)?;
    ctx.session_manager
        .reject_tool(&p.session_id, &p.tool_call_id)
        .await?;
    Ok(json!({}))
}
