use crate::{security, telemetry::TelemetryEvent, AppContext};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::warn;

#[derive(Deserialize)]
struct ToolDecisionParams {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(rename = "toolCallId")]
    tool_call_id: String,
}

/// Scopes that are blocked in FORGE and STORM mode.
/// Read-only tools (file_read, grep, glob, web fetch) are permitted.
const BLOCKED_SCOPES_IN_RESTRICTED_MODE: &[&str] = &["file_write", "shell_exec", "git", "unknown"];

pub async fn approve(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: ToolDecisionParams = serde_json::from_value(params)?;

    // ── Mode enforcement (V02.T26) ────────────────────────────────────────────
    // FORGE and STORM sessions must not execute file-write or shell operations.
    if let Ok(Some(session)) = ctx.storage.get_session(&p.session_id).await {
        let mode = session.mode.to_uppercase();
        if mode == "FORGE" || mode == "STORM" {
            // Check what kind of tool this is
            if let Ok(Some(tool_call)) = ctx.storage.get_tool_call(&p.tool_call_id).await {
                let scope = tool_scope(&tool_call.name);
                if BLOCKED_SCOPES_IN_RESTRICTED_MODE.contains(&scope) {
                    anyhow::bail!(
                        "MODE_VIOLATION: {} — tool '{}' (scope: {scope}) is blocked in {mode} mode. \
                         Use session.setMode to switch to CRUNCH or NORMAL before approving file-write tools.",
                        mode, tool_call.name
                    );
                }
            }
        }
    }

    // Check session permission scopes + security allowlist/denylist before approving (DC.T40)
    if let Ok(Some(tool_call)) = ctx.storage.get_tool_call(&p.tool_call_id).await {
        ctx.session_manager
            .check_tool_permission(&p.session_id, &tool_call.name)
            .await?;

        // Security config gating (DC.T40)
        let sanitized = security::sanitize_tool_input(&tool_call.input);
        if let Err(e) =
            security::check_tool_call(&tool_call.name, &tool_call.input, &ctx.config.security)
        {
            warn!(
                session_id = %p.session_id,
                tool = %tool_call.name,
                reason = %e,
                "tool call blocked by security config"
            );
            ctx.broadcaster.broadcast(
                "session.toolCallRejected",
                json!({
                    "sessionId": p.session_id,
                    "toolCallId": p.tool_call_id,
                    "toolName": tool_call.name,
                    "reason": e.to_string(),
                }),
            );
            // Log to audit trail
            let _ = ctx
                .storage
                .create_tool_call_event(
                    &p.session_id,
                    &tool_call.name,
                    Some(&sanitized),
                    "rejected",
                    Some(&e.to_string()),
                )
                .await;
            return Err(e);
        }

        // Log approved call to audit trail (DC.T43)
        let _ = ctx
            .storage
            .create_tool_call_event(
                &p.session_id,
                &tool_call.name,
                Some(&sanitized),
                "user",
                None,
            )
            .await;
    }

    ctx.session_manager
        .approve_tool(&p.session_id, &p.tool_call_id)
        .await?;
    ctx.telemetry.send(TelemetryEvent::new("tool.approved"));
    Ok(json!({}))
}

pub async fn reject(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: ToolDecisionParams = serde_json::from_value(params)?;

    // Log rejection to audit trail (DC.T43)
    if let Ok(Some(tool_call)) = ctx.storage.get_tool_call(&p.tool_call_id).await {
        let sanitized = security::sanitize_tool_input(&tool_call.input);
        let _ = ctx
            .storage
            .create_tool_call_event(
                &p.session_id,
                &tool_call.name,
                Some(&sanitized),
                "rejected",
                Some("user rejected"),
            )
            .await;
    }

    ctx.session_manager
        .reject_tool(&p.session_id, &p.tool_call_id)
        .await?;
    ctx.telemetry.send(TelemetryEvent::new("tool.denied"));
    Ok(json!({}))
}

/// Map a tool name to its permission scope category.
/// Mirrors the logic in `session::tool_name_to_scope`.
fn tool_scope(tool_name: &str) -> &'static str {
    let lower = tool_name.to_lowercase();
    if lower.contains("read")
        || lower.contains("glob")
        || lower.contains("grep")
        || lower.contains("fetch")
        || lower.contains("search")
    {
        "file_read"
    } else if lower.contains("write") || lower.contains("edit") || lower.contains("notebook") {
        "file_write"
    } else if lower.contains("git") {
        "git"
    } else if lower.contains("bash")
        || lower.contains("shell")
        || lower.contains("exec")
        || lower.contains("run")
        || lower.contains("command")
    {
        "shell_exec"
    } else {
        "unknown"
    }
}
