//! Sprint CC CA.6 — `automation.*` RPC handlers.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

fn automation_to_json(a: &crate::automations::engine::Automation) -> Value {
    json!({
        "name": a.name,
        "description": a.description,
        "enabled": a.enabled,
        "trigger": format!("{:?}", a.trigger).to_lowercase(),
        "condition": a.condition,
        "action": format!("{:?}", a.action).to_lowercase(),
        "actionConfig": a.action_config,
        "builtin": a.builtin,
        "lastTriggeredAt": a.last_triggered_at,
    })
}

/// `automation.list` — returns all automations (built-in + user-configured).
pub async fn list(_params: Value, ctx: AppContext) -> Result<Value> {
    let engine = &ctx.automation_engine;
    let automations = engine.automations.read().await;
    let list: Vec<Value> = automations.iter().map(automation_to_json).collect();
    Ok(json!({ "automations": list }))
}

/// `automation.trigger` — fire a named automation immediately (for testing).
pub async fn trigger(params: Value, ctx: AppContext) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing name"))?
        .to_string();

    let engine = &ctx.automation_engine;
    let automations = engine.automations.read().await;
    let found = automations.iter().any(|a| a.name == name);
    drop(automations);

    if !found {
        anyhow::bail!("automation '{}' not found", name);
    }

    // Fire a synthetic trigger event.
    engine.fire(crate::automations::engine::TriggerEvent {
        kind: crate::automations::engine::TriggerType::SessionComplete,
        session_id: params.get("sessionId").and_then(|v| v.as_str()).map(String::from),
        task_id: None,
        file_path: None,
        session_output: None,
        session_duration_secs: None,
    });

    Ok(json!({ "triggered": name }))
}

/// `automation.disable` — enable or disable an automation by name.
pub async fn disable(params: Value, ctx: AppContext) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing name"))?;
    let enabled = params
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let engine = &ctx.automation_engine;
    let mut automations = engine.automations.write().await;
    let found = automations.iter_mut().find(|a| a.name == name);
    match found {
        Some(a) => {
            a.enabled = enabled;
            Ok(json!({ "name": a.name, "enabled": a.enabled }))
        }
        None => anyhow::bail!("automation '{}' not found", name),
    }
}
