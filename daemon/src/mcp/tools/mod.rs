/// MCP `tools/list` handler — exposes ClawDE task-management tools as MCP tool definitions.
///
/// Each tool definition follows the JSON Schema convention for `inputSchema`.
/// Agents call `tools/list` to discover available tools, then invoke them via
/// `tools/call` (dispatched by `mcp::dispatch`).
///
/// Tool implementation submodules:
/// - `task` — create_task, claim_task, log_event, run_tests, request_approval, transition_task
/// - `patch` — apply_patch (with idempotency key store)
pub mod patch;
pub mod task;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ─── Tool definition type ─────────────────────────────────────────────────────

/// A single MCP tool definition, as returned in `tools/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

impl McpToolDef {
    fn new(name: &str, description: &str, input_schema: Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

// ─── Tool catalogue ───────────────────────────────────────────────────────────

/// Returns all ClawDE tools available via MCP.
///
/// These are defined as a function (not a static) because `serde_json::json!`
/// produces a non-`const` `Value`.  Call once at startup or inline — the list
/// is small and cheap to allocate.
pub fn clawd_tools() -> Vec<McpToolDef> {
    vec![
        // ── create_task ───────────────────────────────────────────────────────
        McpToolDef::new(
            "create_task",
            "Create a new agent task in the ClawDE task queue.",
            json!({
                "type": "object",
                "required": ["title", "repo"],
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Short task title (50 chars max)."
                    },
                    "repo": {
                        "type": "string",
                        "description": "Absolute path to the git repo this task is scoped to."
                    },
                    "summary": {
                        "type": "string",
                        "description": "Detailed description of what needs to be done."
                    },
                    "acceptance_criteria": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of acceptance criteria. Every task should have at least one."
                    },
                    "priority": {
                        "type": "string",
                        "enum": ["low", "medium", "high", "critical"],
                        "description": "Task priority. Defaults to 'medium'.",
                        "default": "medium"
                    },
                    "labels": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional labels/tags for filtering."
                    }
                },
                "additionalProperties": false
            }),
        ),

        // ── claim_task ────────────────────────────────────────────────────────
        McpToolDef::new(
            "claim_task",
            "Claim a pending task for the calling agent. Returns an error if already claimed.",
            json!({
                "type": "object",
                "required": ["task_id"],
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "UUID of the task to claim."
                    }
                },
                "additionalProperties": false
            }),
        ),

        // ── log_event ─────────────────────────────────────────────────────────
        McpToolDef::new(
            "log_event",
            "Append a structured event to the task's activity log.",
            json!({
                "type": "object",
                "required": ["task_id", "event_type", "data"],
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "UUID of the task this event belongs to."
                    },
                    "event_type": {
                        "type": "string",
                        "description": "Event type identifier, e.g. 'progress', 'error', 'test_result'."
                    },
                    "data": {
                        "type": "object",
                        "description": "Arbitrary structured payload for this event."
                    }
                },
                "additionalProperties": false
            }),
        ),

        // ── apply_patch ───────────────────────────────────────────────────────
        McpToolDef::new(
            "apply_patch",
            "Apply a unified-diff patch to the task's worktree. Idempotent via idempotency_key.",
            json!({
                "type": "object",
                "required": ["task_id", "patch", "idempotency_key"],
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "UUID of the Active+Claimed task."
                    },
                    "patch": {
                        "type": "string",
                        "description": "Unified diff string (output of `git diff` or `diff -u`)."
                    },
                    "idempotency_key": {
                        "type": "string",
                        "description": "UUID v4 generated by the caller. Re-sending the same key is a no-op."
                    }
                },
                "additionalProperties": false
            }),
        ),

        // ── run_tests ─────────────────────────────────────────────────────────
        McpToolDef::new(
            "run_tests",
            "Run the test suite in the task's worktree. Idempotent via idempotency_key.",
            json!({
                "type": "object",
                "required": ["task_id", "idempotency_key"],
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "UUID of the Active+Claimed task."
                    },
                    "command": {
                        "type": "string",
                        "description": "Test command to run, e.g. 'cargo test' or 'pnpm test'. Defaults to 'cargo test'.",
                        "default": "cargo test"
                    },
                    "idempotency_key": {
                        "type": "string",
                        "description": "UUID v4. Re-sending the same key returns the cached result."
                    }
                },
                "additionalProperties": false
            }),
        ),

        // ── request_approval ──────────────────────────────────────────────────
        McpToolDef::new(
            "request_approval",
            "Request human approval before executing a high-risk tool call. Sends a push notification to connected clients.",
            json!({
                "type": "object",
                "required": ["task_id", "tool_name", "arguments", "risk_level"],
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "UUID of the task requesting approval."
                    },
                    "tool_name": {
                        "type": "string",
                        "description": "The tool the agent wants to call (e.g. 'apply_patch', 'run_tests')."
                    },
                    "arguments": {
                        "type": "object",
                        "description": "Arguments the agent intends to pass to the tool."
                    },
                    "risk_level": {
                        "type": "string",
                        "enum": ["low", "medium", "high", "critical"],
                        "description": "Assessed risk level. Anything 'high' or 'critical' requires approval."
                    }
                },
                "additionalProperties": false
            }),
        ),

        // ── transition_task ───────────────────────────────────────────────────
        McpToolDef::new(
            "transition_task",
            "Transition a task to a new state (e.g., 'in_progress' → 'done', 'blocked').",
            json!({
                "type": "object",
                "required": ["task_id", "new_state"],
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "UUID of the task to transition."
                    },
                    "new_state": {
                        "type": "string",
                        "enum": ["pending", "in_progress", "done", "blocked", "interrupted"],
                        "description": "Target state."
                    },
                    "reason": {
                        "type": "string",
                        "description": "Human-readable reason for the transition. Required when transitioning to 'done' or 'blocked'."
                    }
                },
                "additionalProperties": false
            }),
        ),
    ]
}

// ─── tools/list handler ───────────────────────────────────────────────────────

/// Handle a MCP `tools/list` request.
///
/// Returns `{"tools": [...]}` as a `serde_json::Value` ready to embed in a
/// `McpResponse::ok(id, handle_tools_list())`.
pub fn handle_tools_list() -> Value {
    let tools = clawd_tools();
    json!({ "tools": tools })
}
