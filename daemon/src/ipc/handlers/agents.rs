use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

pub async fn register(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent_id = params.get("agent_id").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;
    let agent_type = params.get("agent_type").and_then(|v| v.as_str()).unwrap_or("claude");
    let session_id = params.get("session_id").and_then(|v| v.as_str());
    let repo_path = params.get("repo_path").and_then(|v| v.as_str()).unwrap_or("");

    let agent = ctx.task_storage.register_agent(agent_id, agent_type, session_id, repo_path).await?;

    let _ = ctx.task_storage.log_activity(
        agent_id, None, None,
        "session_start", "system",
        Some("Agent registered"),
        None, repo_path,
    ).await;

    ctx.broadcaster.broadcast("agent.connected", json!({
        "agent_id": agent_id,
        "agent_type": agent_type,
    }));

    Ok(json!({ "agent": agent }))
}

pub async fn list(params: Value, ctx: &AppContext) -> Result<Value> {
    let repo_path = params.get("repo_path").and_then(|v| v.as_str());
    let agents = ctx.task_storage.list_agents(repo_path).await?;
    Ok(json!({ "agents": agents }))
}

pub async fn heartbeat(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent_id = params.get("agent_id").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;
    ctx.task_storage.update_agent_heartbeat(agent_id).await?;
    Ok(json!({ "ok": true }))
}

pub async fn disconnect(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent_id = params.get("agent_id").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;
    ctx.task_storage.mark_agent_disconnected(agent_id).await?;
    ctx.broadcaster.broadcast("agent.disconnected", json!({ "agent_id": agent_id }));
    Ok(json!({ "ok": true }))
}
