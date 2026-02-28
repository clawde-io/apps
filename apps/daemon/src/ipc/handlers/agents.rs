use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

pub async fn register(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent_id = params
        .get("agent_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;
    let agent_type = params
        .get("agent_type")
        .and_then(|v| v.as_str())
        .unwrap_or("claude");
    let session_id = params.get("session_id").and_then(|v| v.as_str());
    let repo_path = params
        .get("repo_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let agent = ctx
        .task_storage
        .register_agent(agent_id, agent_type, session_id, repo_path)
        .await?;

    let _ = ctx
        .task_storage
        .log_activity(
            agent_id,
            None,
            None,
            "session_start",
            "system",
            Some("Agent registered"),
            None,
            repo_path,
        )
        .await;

    ctx.broadcaster.broadcast(
        "agent.connected",
        json!({
            "agent_id": agent_id,
            "agent_type": agent_type,
        }),
    );

    Ok(json!({ "agent": agent }))
}

pub async fn list(params: Value, ctx: &AppContext) -> Result<Value> {
    let repo_path = params.get("repo_path").and_then(|v| v.as_str());
    let agents = ctx.task_storage.list_agents(repo_path).await?;
    Ok(json!({ "agents": agents }))
}

pub async fn heartbeat(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent_id = params
        .get("agent_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;
    ctx.task_storage.update_agent_heartbeat(agent_id).await?;
    Ok(json!({ "ok": true }))
}

pub async fn disconnect(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent_id = params
        .get("agent_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;
    ctx.task_storage.mark_agent_disconnected(agent_id).await?;
    ctx.broadcaster
        .broadcast("agent.disconnected", json!({ "agent_id": agent_id }));
    Ok(json!({ "ok": true }))
}

// ─── Phase 43e: Orchestration handlers ───────────────────────────────────────

/// `agents.spawn` — spawn a new orchestrated agent.
///
/// Params: `{ role, task_id, complexity?, worktree_path?, previous_provider? }`
pub async fn spawn_agent(params: Value, ctx: &AppContext) -> Result<Value> {
    let role_str = params
        .get("role")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing role"))?;
    let task_id = params
        .get("task_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing task_id"))?;
    let complexity = params
        .get("complexity")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");
    let worktree_path = params
        .get("worktree_path")
        .and_then(|v| v.as_str())
        .map(String::from);
    let previous_provider = params
        .get("previous_provider")
        .and_then(|v| v.as_str())
        .and_then(|s| match s {
            "claude" => Some(crate::agents::capabilities::Provider::Claude),
            "codex" => Some(crate::agents::capabilities::Provider::Codex),
            _ => None,
        });

    let role = crate::agents::roles::AgentRole::from_str(role_str)
        .ok_or_else(|| anyhow::anyhow!("unknown role: {}", role_str))?;

    let agent_id = ctx
        .orchestrator
        .spawn(role, task_id, complexity, worktree_path, previous_provider)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    ctx.broadcaster.broadcast(
        "agent.spawned",
        json!({ "agent_id": agent_id, "task_id": task_id, "role": role_str }),
    );

    Ok(json!({ "agent_id": agent_id }))
}

/// `agents.list` — list agents, optionally filtered by task or role.
///
/// Params: `{ task_id?, role? }`
pub async fn list_orchestrated(params: Value, ctx: &AppContext) -> Result<Value> {
    let registry = ctx.orchestrator.registry.read().await;

    let agents: Vec<_> = if let Some(task_id) = params.get("task_id").and_then(|v| v.as_str()) {
        registry
            .list_by_task(task_id)
            .into_iter()
            .cloned()
            .collect()
    } else {
        registry.list_active().into_iter().cloned().collect()
    };

    Ok(json!({ "agents": agents }))
}

/// `agents.cancel` — cancel an orchestrated agent.
///
/// Params: `{ agent_id }`
pub async fn cancel_agent(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent_id = params
        .get("agent_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;

    let ok = ctx.orchestrator.cancel(agent_id).await;
    if ok {
        ctx.broadcaster
            .broadcast("agent.canceled", json!({ "agent_id": agent_id }));
    }
    Ok(json!({ "ok": ok }))
}

/// `agents.heartbeat` — orchestrated agent sends a heartbeat.
///
/// Params: `{ agent_id, tokens_used?, cost_usd? }`
pub async fn orchestrator_heartbeat(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent_id = params
        .get("agent_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;
    let tokens_used = params.get("tokens_used").and_then(|v| v.as_u64());
    let cost_usd = params.get("cost_usd").and_then(|v| v.as_f64());

    let ok = ctx
        .orchestrator
        .heartbeat(agent_id, tokens_used, cost_usd)
        .await;
    Ok(json!({ "ok": ok }))
}
