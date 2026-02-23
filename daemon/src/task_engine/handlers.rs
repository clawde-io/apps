//! Task Engine RPC handlers — Phase 45.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

use super::storage::TaskEngineStorage;

fn te_storage(ctx: &AppContext) -> TaskEngineStorage {
    TaskEngineStorage::new(ctx.storage.pool())
}

// ─── Phase handlers ───────────────────────────────────────────────────────────

/// `te.phase.create` — create a new phase.
pub async fn phase_create(params: Value, ctx: &AppContext) -> Result<Value> {
    let display_id = params["displayId"].as_str().unwrap_or("P0").to_string();
    let title = params["title"].as_str().unwrap_or("").to_string();
    let description = params["description"].as_str().unwrap_or("").to_string();
    let priority = params["priority"].as_str().unwrap_or("medium").to_string();
    let planning_doc = params["planningDocPath"].as_str().map(str::to_string);
    let repo = params["repo"].as_str().map(str::to_string);

    let phase = te_storage(ctx)
        .create_phase(
            &display_id,
            &title,
            &description,
            &priority,
            planning_doc.as_deref(),
            repo.as_deref(),
        )
        .await?;

    Ok(serde_json::to_value(&phase)?)
}

/// `te.phase.list` — list all phases.
pub async fn phase_list(_params: Value, ctx: &AppContext) -> Result<Value> {
    let phases = te_storage(ctx).list_phases().await?;
    Ok(json!({ "phases": phases }))
}

// ─── Task handlers ────────────────────────────────────────────────────────────

/// `te.task.create` — create a task in a phase.
pub async fn task_create(params: Value, ctx: &AppContext) -> Result<Value> {
    let display_id = params["displayId"].as_str().unwrap_or("").to_string();
    let phase_id = params["phaseId"].as_str().unwrap_or("").to_string();
    let parent_task_id = params["parentTaskId"].as_str().map(str::to_string);
    let title = params["title"].as_str().unwrap_or("").to_string();
    let description = params["description"].as_str().unwrap_or("").to_string();
    let task_type = params["taskType"].as_str().unwrap_or("implementation").to_string();
    let priority = params["priority"].as_str().unwrap_or("medium").to_string();

    let task = te_storage(ctx)
        .create_task(
            &display_id,
            &phase_id,
            parent_task_id.as_deref(),
            &title,
            &description,
            &task_type,
            &priority,
        )
        .await?;

    Ok(serde_json::to_value(&task)?)
}

/// `te.task.get` — get a task by ID.
pub async fn task_get(params: Value, ctx: &AppContext) -> Result<Value> {
    let id = params["id"].as_str().unwrap_or("").to_string();
    let task = te_storage(ctx).get_task(&id).await?;
    Ok(serde_json::to_value(&task)?)
}

/// `te.task.list` — list tasks with optional phase/status filter.
pub async fn task_list(params: Value, ctx: &AppContext) -> Result<Value> {
    let phase_id = params["phaseId"].as_str().map(str::to_string);
    let status = params["status"].as_str().map(str::to_string);
    let tasks = te_storage(ctx)
        .list_tasks(phase_id.as_deref(), status.as_deref())
        .await?;
    Ok(json!({ "tasks": tasks }))
}

/// `te.task.transition` — change a task's status.
pub async fn task_transition(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id = params["taskId"].as_str().unwrap_or("").to_string();
    let new_status = params["status"].as_str().unwrap_or("").to_string();
    let reason = params["reason"].as_str().map(str::to_string);

    let task = te_storage(ctx)
        .transition_task(&task_id, &new_status, reason.as_deref())
        .await?;

    Ok(serde_json::to_value(&task)?)
}

// ─── Agent handlers ───────────────────────────────────────────────────────────

/// `te.agent.register` — register a new agent.
pub async fn agent_register(params: Value, ctx: &AppContext) -> Result<Value> {
    let name = params["name"].as_str().unwrap_or("unnamed").to_string();
    let agent_type = params["agentType"].as_str().unwrap_or("claude").to_string();
    let role = params["role"].as_str().unwrap_or("implementer").to_string();
    let caps = serde_json::to_string(&params["capabilities"]).unwrap_or_else(|_| "[]".into());
    let model_id = params["modelId"].as_str().map(str::to_string);
    let max_ctx = params["maxContextTokens"].as_i64();

    let agent = te_storage(ctx)
        .register_agent(&name, &agent_type, &role, &caps, model_id.as_deref(), max_ctx)
        .await?;

    Ok(json!({
        "agentId": agent.id,
        "heartbeatIntervalSecs": agent.heartbeat_interval_secs,
    }))
}

/// `te.agent.heartbeat` — update agent heartbeat.
pub async fn agent_heartbeat(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent_id = params["agentId"].as_str().unwrap_or("").to_string();
    let status = params["status"].as_str().unwrap_or("working").to_string();
    let current_task_id = params["currentTaskId"].as_str().map(str::to_string);

    te_storage(ctx)
        .heartbeat(&agent_id, &status, current_task_id.as_deref())
        .await?;

    Ok(json!({ "ok": true }))
}

/// `te.agent.deregister` — agent going offline.
pub async fn agent_deregister(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent_id = params["agentId"].as_str().unwrap_or("").to_string();
    te_storage(ctx).deregister_agent(&agent_id).await?;
    Ok(json!({ "ok": true }))
}

// ─── Task claiming handlers ───────────────────────────────────────────────────

/// `te.task.claim` — atomically claim a task.
pub async fn task_claim(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent_id = params["agentId"].as_str().unwrap_or("").to_string();
    let s = te_storage(ctx);

    // Specific task claim
    if let Some(task_id) = params["taskId"].as_str() {
        let result = s.claim_task(task_id, &agent_id).await?;
        if let Some(task) = result {
            let checkpoint = s.latest_checkpoint(&task.id).await?;
            return Ok(json!({
                "claimed": true,
                "task": task,
                "lastCheckpoint": checkpoint,
            }));
        } else {
            return Ok(json!({
                "claimed": false,
                "reason": "already_claimed_or_not_available",
            }));
        }
    }

    // Next available claim
    let role = params["role"].as_str().unwrap_or("implementer").to_string();
    let result = s.claim_next_task(&agent_id, &role, None).await?;

    match result {
        Some(task) => {
            let checkpoint = s.latest_checkpoint(&task.id).await?;
            Ok(json!({
                "claimed": true,
                "task": task,
                "lastCheckpoint": checkpoint,
            }))
        }
        None => Ok(json!({
            "claimed": false,
            "reason": "no_available_tasks",
        })),
    }
}

// ─── Event handlers ───────────────────────────────────────────────────────────

/// `te.event.log` — append an event to a task's event log.
pub async fn event_log(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id = params["taskId"].as_str().unwrap_or("").to_string();
    let agent_id = params["agentId"].as_str().map(str::to_string);
    let event_type = params["eventType"].as_str().unwrap_or("note.added").to_string();
    let payload = serde_json::to_string(&params["payload"]).unwrap_or_else(|_| "{}".into());
    let idem_key = params["idempotencyKey"].as_str().map(str::to_string);

    let event = te_storage(ctx)
        .append_event(
            &task_id,
            agent_id.as_deref(),
            &event_type,
            &payload,
            idem_key.as_deref(),
        )
        .await?;

    Ok(serde_json::to_value(&event)?)
}

/// `te.event.list` — list events for a task.
pub async fn event_list(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id = params["taskId"].as_str().unwrap_or("").to_string();
    let limit = params["limit"].as_i64().unwrap_or(100);
    let events = te_storage(ctx).list_events(&task_id, limit).await?;
    Ok(json!({ "events": events }))
}

// ─── Checkpoint handlers ──────────────────────────────────────────────────────

/// `te.checkpoint.write` — write a checkpoint for a task.
pub async fn checkpoint_write(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id = params["taskId"].as_str().unwrap_or("").to_string();
    let agent_id = params["agentId"].as_str().unwrap_or("").to_string();
    let cp_type = params["checkpointType"].as_str().unwrap_or("periodic").to_string();
    let current_action = params["currentAction"].as_str().unwrap_or("").to_string();
    let completed = serde_json::to_string(&params["completedItems"]).unwrap_or_else(|_| "[]".into());
    let files = serde_json::to_string(&params["filesModified"]).unwrap_or_else(|_| "[]".into());
    let next = serde_json::to_string(&params["nextSteps"]).unwrap_or_else(|_| "[]".into());
    let remaining = serde_json::to_string(&params["remainingItems"]).unwrap_or_else(|_| "[]".into());
    let context_summary = params["contextSummary"].as_str().map(str::to_string);

    let cp = te_storage(ctx)
        .write_checkpoint(
            &task_id,
            &agent_id,
            &cp_type,
            &current_action,
            &completed,
            &files,
            &next,
            &remaining,
            context_summary.as_deref(),
        )
        .await?;

    Ok(serde_json::to_value(&cp)?)
}

// ─── Note handlers ────────────────────────────────────────────────────────────

/// `te.note.add` — add a note to a task.
pub async fn note_add(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id = params["taskId"].as_str().unwrap_or("").to_string();
    let agent_id = params["agentId"].as_str().map(str::to_string);
    let note_type = params["noteType"].as_str().unwrap_or("observation").to_string();
    let title = params["title"].as_str().unwrap_or("").to_string();
    let content = params["content"].as_str().unwrap_or("").to_string();
    let related_file = params["relatedFile"].as_str().map(str::to_string);
    let visibility = params["visibility"].as_str().unwrap_or("team").to_string();

    let note = te_storage(ctx)
        .add_note(
            &task_id,
            agent_id.as_deref(),
            &note_type,
            &title,
            &content,
            related_file.as_deref(),
            &visibility,
        )
        .await?;

    Ok(serde_json::to_value(&note)?)
}

/// `te.note.list` — list notes for a task.
pub async fn note_list(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id = params["taskId"].as_str().unwrap_or("").to_string();
    let notes = te_storage(ctx).list_notes(&task_id).await?;
    Ok(json!({ "notes": notes }))
}
