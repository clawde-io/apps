//! Role-Based Access Control (RBAC) for MCP tool dispatch.
//!
//! Each agent is assigned a `AgentRole`. The role determines which tools the
//! agent is allowed to invoke. An `Implementer` has full access; other roles
//! have narrower privileges aligned with their function.

use super::sandbox::PolicyViolation;

// ─── Agent roles ──────────────────────────────────────────────────────────────

/// Roles that an AI agent can hold.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentRole {
    /// Routes tasks and coordinates work — minimal tool access.
    Router,
    /// Plans and decomposes tasks — read-only access.
    Planner,
    /// Writes and modifies code — full tool access.
    Implementer,
    /// Reviews code — read-only access plus logging.
    Reviewer,
    /// Runs quality assurance — test execution and state transitions.
    QaExecutor,
    /// Unknown or unregistered role — no tool access.
    Unknown,
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AgentRole::Router => "router",
            AgentRole::Planner => "planner",
            AgentRole::Implementer => "implementer",
            AgentRole::Reviewer => "reviewer",
            AgentRole::QaExecutor => "qa_executor",
            AgentRole::Unknown => "unknown",
        };
        write!(f, "{}", s)
    }
}

impl AgentRole {
    /// Parse an agent role from a string identifier.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "router" => AgentRole::Router,
            "planner" => AgentRole::Planner,
            "implementer" => AgentRole::Implementer,
            "reviewer" => AgentRole::Reviewer,
            "qa_executor" => AgentRole::QaExecutor,
            _ => AgentRole::Unknown,
        }
    }
}

// ─── Role → allowed tools table ──────────────────────────────────────────────

/// Static table mapping each role to its permitted tool set.
///
/// `Implementer` is represented as `None` (all tools allowed).
/// All other roles have explicit allow-lists.
pub const ROLE_ALLOWED_TOOLS: &[(AgentRole, Option<&[&str]>)] = &[
    (
        AgentRole::Router,
        Some(&["create_task", "transition_task", "log_event"]),
    ),
    (
        AgentRole::Planner,
        Some(&["read_file", "search_files", "log_event"]),
    ),
    (
        AgentRole::Implementer,
        None, // all tools allowed
    ),
    (
        AgentRole::Reviewer,
        Some(&["read_file", "search_files", "log_event"]),
    ),
    (
        AgentRole::QaExecutor,
        Some(&["run_tests", "log_event", "transition_task"]),
    ),
    (
        AgentRole::Unknown,
        Some(&[]), // no tools
    ),
];

// ─── RBAC check ───────────────────────────────────────────────────────────────

/// Check whether `role` is authorised to invoke `tool`.
///
/// Returns `Ok(())` when permitted, or
/// `Err(PolicyViolation::UnauthorizedTool)` when denied.
pub fn check_tool_authorized(role: &AgentRole, tool: &str) -> Result<(), PolicyViolation> {
    for (entry_role, allowed) in ROLE_ALLOWED_TOOLS {
        if entry_role != role {
            continue;
        }

        return match allowed {
            None => Ok(()), // Implementer: all tools allowed.
            Some(tools) => {
                if tools.contains(&tool) {
                    Ok(())
                } else {
                    Err(PolicyViolation::UnauthorizedTool {
                        tool: tool.to_string(),
                        role: role.to_string(),
                    })
                }
            }
        };
    }

    // Role not in table (shouldn't happen, but treat as Unknown).
    Err(PolicyViolation::UnauthorizedTool {
        tool: tool.to_string(),
        role: role.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_can_create_task() {
        assert!(check_tool_authorized(&AgentRole::Router, "create_task").is_ok());
    }

    #[test]
    fn router_cannot_apply_patch() {
        let result = check_tool_authorized(&AgentRole::Router, "apply_patch");
        assert!(matches!(result, Err(PolicyViolation::UnauthorizedTool { .. })));
    }

    #[test]
    fn planner_can_read_file() {
        assert!(check_tool_authorized(&AgentRole::Planner, "read_file").is_ok());
    }

    #[test]
    fn planner_cannot_apply_patch() {
        let result = check_tool_authorized(&AgentRole::Planner, "apply_patch");
        assert!(matches!(result, Err(PolicyViolation::UnauthorizedTool { .. })));
    }

    #[test]
    fn implementer_can_do_anything() {
        assert!(check_tool_authorized(&AgentRole::Implementer, "apply_patch").is_ok());
        assert!(check_tool_authorized(&AgentRole::Implementer, "run_tests").is_ok());
        assert!(check_tool_authorized(&AgentRole::Implementer, "git_push").is_ok());
    }

    #[test]
    fn reviewer_cannot_patch() {
        let result = check_tool_authorized(&AgentRole::Reviewer, "apply_patch");
        assert!(matches!(result, Err(PolicyViolation::UnauthorizedTool { .. })));
    }

    #[test]
    fn qa_executor_can_run_tests() {
        assert!(check_tool_authorized(&AgentRole::QaExecutor, "run_tests").is_ok());
    }

    #[test]
    fn qa_executor_cannot_apply_patch() {
        let result = check_tool_authorized(&AgentRole::QaExecutor, "apply_patch");
        assert!(matches!(result, Err(PolicyViolation::UnauthorizedTool { .. })));
    }

    #[test]
    fn unknown_role_denied_everything() {
        let result = check_tool_authorized(&AgentRole::Unknown, "read_file");
        assert!(matches!(result, Err(PolicyViolation::UnauthorizedTool { .. })));
    }

    #[test]
    fn role_from_str() {
        assert_eq!(AgentRole::from_str("planner"), AgentRole::Planner);
        assert_eq!(AgentRole::from_str("implementer"), AgentRole::Implementer);
        assert_eq!(AgentRole::from_str("unknown_xyz"), AgentRole::Unknown);
    }
}
