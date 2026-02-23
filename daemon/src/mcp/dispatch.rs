/// MCP `tools/call` dispatcher — routes tool invocations to internal handlers.
///
/// `McpDispatcher` holds a reference to `AppContext` and maps tool names to
/// the handler functions in `mcp::tools::*`.  Write tools (apply_patch,
/// run_tests) verify that the referenced task is Active+Claimed before
/// proceeding; all other tools are callable in any task state.
use crate::AppContext;
use serde_json::Value;
use std::sync::Arc;
use tracing::{info, warn};

use super::transport::{McpError, MCP_INVALID_PARAMS, MCP_PROVIDER_NOT_AVAILABLE};
use super::tools as tool_list;

/// Write tools that require the task to be Active+Claimed before proceeding.
/// `transition_task` and `claim_task` are included so agents cannot advance
/// task state or claim tasks without going through proper ownership checks.
const WRITE_TOOLS: &[&str] = &["apply_patch", "run_tests", "transition_task", "claim_task"];

pub struct McpDispatcher {
    ctx: Arc<AppContext>,
}

impl McpDispatcher {
    pub fn new(ctx: Arc<AppContext>) -> Self {
        Self { ctx }
    }

    /// Dispatch a `tools/call` invocation.
    ///
    /// `tool_name`  — the `name` field from the MCP `tools/call` params.
    /// `arguments`  — the `arguments` object from the MCP `tools/call` params.
    /// `agent_id`   — optional agent identifier from the calling session.
    ///
    /// Returns `Ok(Value)` with the tool result, or `Err(anyhow::Error)` whose
    /// message encodes a MCP error code (e.g. `"MCP_INVALID_PARAMS: ..."` or
    /// `"MCP_PROVIDER_NOT_AVAILABLE: ..."`) so callers can map it correctly.
    pub async fn dispatch(
        &self,
        tool_name: &str,
        arguments: Value,
        agent_id: Option<String>,
    ) -> anyhow::Result<Value> {
        // Verify the tool is in our catalogue first.
        let known = tool_list::clawd_tools()
            .into_iter()
            .any(|t| t.name == tool_name);
        if !known {
            let msg = format!("unknown tool: {}", tool_name);
            warn!(tool = tool_name, "MCP unknown tool");
            return Err(anyhow::anyhow!("MCP_INVALID_PARAMS: {}", msg));
        }

        // For write tools, verify the task is Active+Claimed.
        if WRITE_TOOLS.contains(&tool_name) {
            self.verify_active_claimed(&arguments, agent_id.as_deref())
                .await?;
        }

        // Route to the correct handler.
        let result = match tool_name {
            "create_task" => {
                super::tools::task::create_task(&self.ctx, arguments).await?
            }
            "claim_task" => {
                super::tools::task::claim_task(&self.ctx, arguments, agent_id.as_deref()).await?
            }
            "log_event" => {
                super::tools::task::log_event(&self.ctx, arguments, agent_id.as_deref()).await?
            }
            "apply_patch" => {
                super::tools::patch::apply_patch(&self.ctx, arguments, agent_id.as_deref()).await?
            }
            "run_tests" => {
                super::tools::task::run_tests(&self.ctx, arguments, agent_id.as_deref()).await?
            }
            "request_approval" => {
                super::tools::task::request_approval(&self.ctx, arguments, agent_id.as_deref())
                    .await?
            }
            "transition_task" => {
                super::tools::task::transition_task(&self.ctx, arguments, agent_id.as_deref())
                    .await?
            }
            other => {
                // Should not reach here — already checked above.
                return Err(anyhow::anyhow!("MCP_INVALID_PARAMS: unknown tool: {}", other));
            }
        };

        // Emit an audit event (stub — log only; full audit pipeline TBD).
        info!(
            tool = tool_name,
            agent = agent_id.as_deref().unwrap_or("unknown"),
            "MCP tool executed"
        );

        Ok(result)
    }

    /// Verify that the `task_id` in `arguments` corresponds to a task that is
    /// currently `in_progress` and claimed by `agent_id`.
    ///
    /// Returns `Err` with a `MCP_PROVIDER_NOT_AVAILABLE` message if the task is
    /// not in an Active+Claimed state, so the caller can map it to error -32002.
    async fn verify_active_claimed(
        &self,
        arguments: &Value,
        agent_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let task_id = arguments
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!("MCP_INVALID_PARAMS: missing required field 'task_id'")
            })?;

        let task = self
            .ctx
            .task_storage
            .get_task(task_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("MCP_PROVIDER_NOT_AVAILABLE: task '{}' not found", task_id)
            })?;

        if task.status != "in_progress" {
            return Err(anyhow::anyhow!(
                "MCP_PROVIDER_NOT_AVAILABLE: task '{}' is in state '{}' — must be 'in_progress' (Active+Claimed)",
                task_id,
                task.status
            ));
        }

        // If agent_id is provided, verify ownership.
        if let Some(aid) = agent_id {
            if task.claimed_by.as_deref() != Some(aid) {
                return Err(anyhow::anyhow!(
                    "MCP_PROVIDER_NOT_AVAILABLE: task '{}' is claimed by '{}', not '{}'",
                    task_id,
                    task.claimed_by.as_deref().unwrap_or("unknown"),
                    aid
                ));
            }
        }

        Ok(())
    }

    /// Convert an `anyhow::Error` returned from `dispatch` into a `McpError`
    /// with the correct code.  This is a helper for the MCP message loop.
    pub fn classify_error(err: &anyhow::Error) -> McpError {
        let msg = err.to_string();
        if msg.starts_with("MCP_INVALID_PARAMS:") {
            let detail = msg.trim_start_matches("MCP_INVALID_PARAMS:").trim();
            McpError::new(MCP_INVALID_PARAMS, detail)
        } else if msg.starts_with("MCP_PROVIDER_NOT_AVAILABLE:") {
            let detail = msg
                .trim_start_matches("MCP_PROVIDER_NOT_AVAILABLE:")
                .trim();
            McpError::new(MCP_PROVIDER_NOT_AVAILABLE, detail)
        } else {
            McpError::new(-32603, "internal error")
        }
    }
}
