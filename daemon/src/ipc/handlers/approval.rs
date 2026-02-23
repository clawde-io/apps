//! RPC handlers for the human-approval workflow.
//!
//! Exposes:
//!   `approval.list`    — list tasks currently awaiting human approval
//!   `approval.respond` — grant or deny a pending approval request

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

fn sv<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|v| v.as_str())
}

/// `approval.list` — list tasks in `needs_approval` state.
///
/// Params: (none required)
/// Returns: `{ approvals: [ { task_id, pending_approval_id, tool_name, risk_level, ... } ] }`
pub async fn list(_params: Value, ctx: &AppContext) -> Result<Value> {
    use crate::tasks::storage::TaskListParams;

    let tasks = ctx
        .task_storage
        .list_tasks(&TaskListParams {
            status: Some("needs_approval".to_string()),
            limit: Some(100),
            ..Default::default()
        })
        .await?;

    let approvals: Vec<Value> = tasks
        .into_iter()
        .map(|t| {
            json!({
                "task_id": t.id,
                "title": t.title,
                "claimed_by": t.claimed_by,
                "repo_path": t.repo_path,
                "updated_at": t.updated_at,
                // pending_approval_id is stored in notes by request_approval
                "approval_id": t.notes,
            })
        })
        .collect();

    Ok(json!({ "approvals": approvals }))
}

/// `approval.respond` — grant or deny a pending approval request.
///
/// Params: `{ approval_id: string, decision: "grant" | "deny", reason?: string }`
///
/// Finds the task awaiting approval with the given `approval_id` and transitions
/// it: `grant` → `in_progress` (Active), `deny` → `blocked`.
pub async fn respond(params: Value, ctx: &AppContext) -> Result<Value> {
    let approval_id =
        sv(&params, "approval_id").ok_or_else(|| anyhow::anyhow!("missing field: approval_id"))?;
    let decision = sv(&params, "decision")
        .ok_or_else(|| anyhow::anyhow!("missing field: decision (must be 'grant' or 'deny')"))?;
    let reason = sv(&params, "reason").unwrap_or("user decision");

    if decision != "grant" && decision != "deny" {
        return Err(anyhow::anyhow!(
            "invalid decision '{}' — must be 'grant' or 'deny'",
            decision
        ));
    }

    // Find the task whose notes field contains this approval_id.
    // (set by request_approval when it transitions the task to needs_approval)
    use crate::tasks::storage::TaskListParams;
    let candidates = ctx
        .task_storage
        .list_tasks(&TaskListParams {
            status: Some("needs_approval".to_string()),
            limit: Some(200),
            ..Default::default()
        })
        .await?;

    let task = candidates
        .into_iter()
        .find(|t| t.notes.as_deref() == Some(approval_id))
        .ok_or_else(|| {
            anyhow::anyhow!("approval '{}' not found or already resolved", approval_id)
        })?;

    let task_id = &task.id;

    match decision {
        "grant" => {
            ctx.task_storage
                .update_status(task_id, "in_progress", None, None)
                .await?;
            ctx.broadcaster.broadcast(
                "task.approvalGranted",
                json!({
                    "approval_id": approval_id,
                    "task_id": task_id,
                    "granted_by": "user",
                }),
            );
            tracing::info!(
                approval_id = %approval_id,
                task_id = %task_id,
                "approval granted"
            );
        }
        "deny" => {
            ctx.task_storage
                .update_status(task_id, "blocked", None, Some(reason))
                .await?;
            ctx.broadcaster.broadcast(
                "task.approvalDenied",
                json!({
                    "approval_id": approval_id,
                    "task_id": task_id,
                    "denied_by": "user",
                    "reason": reason,
                }),
            );
            tracing::info!(
                approval_id = %approval_id,
                task_id = %task_id,
                reason = %reason,
                "approval denied"
            );
        }
        _ => unreachable!(),
    }

    Ok(json!({
        "approval_id": approval_id,
        "task_id": task_id,
        "decision": decision,
    }))
}
