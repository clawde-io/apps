//! Approval rule evaluation — determines whether a tool call may proceed
//! immediately, must wait for human approval, or should be denied outright.

use crate::tasks::reducer::TaskState;
use crate::tasks::schema::RiskLevel;

use super::engine::PolicyDecision;

// ─── Approval rules ───────────────────────────────────────────────────────────

/// Configurable approval policy.
///
/// The defaults are conservative: low-risk tools auto-approve, medium-risk
/// tools require an active task, and high/critical tools always need explicit
/// human approval.
#[derive(Debug, Clone)]
pub struct ApprovalRules {
    /// Risk level at which a tool call must wait for approval.
    pub min_risk_for_approval: RiskLevel,
    /// Auto-approve tools whose risk is Low.
    pub auto_approve_low: bool,
    /// Require the task to be Active+Claimed for Medium-risk tools.
    pub require_active_task_for_medium: bool,
}

impl Default for ApprovalRules {
    fn default() -> Self {
        Self {
            min_risk_for_approval: RiskLevel::High,
            auto_approve_low: true,
            require_active_task_for_medium: true,
        }
    }
}

impl ApprovalRules {
    /// Evaluate whether a tool call with `risk` may proceed given the optional
    /// current `task_state`.
    ///
    /// Decision matrix:
    /// - `Low`      → `Allow` always (if `auto_approve_low` is set)
    /// - `Medium`   → `Allow` only when task is `Active`+`Claimed`; otherwise `Deny`
    /// - `High`     → `NeedsApproval`
    /// - `Critical` → `NeedsApproval`
    pub fn should_approve(
        &self,
        tool: &str,
        risk: RiskLevel,
        task_state: Option<&TaskState>,
    ) -> PolicyDecision {
        match risk {
            RiskLevel::Low => {
                if self.auto_approve_low {
                    PolicyDecision::Allow
                } else {
                    PolicyDecision::NeedsApproval {
                        tool: tool.to_string(),
                        risk: RiskLevel::Low,
                        reason: "auto-approve disabled for low-risk tools".to_string(),
                    }
                }
            }

            RiskLevel::Medium => {
                if !self.require_active_task_for_medium {
                    return PolicyDecision::Allow;
                }
                // Medium tools require Active+Claimed task state.
                let allowed = matches!(task_state, Some(TaskState::Active));
                if allowed {
                    PolicyDecision::Allow
                } else {
                    let state_desc = task_state
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "none".to_string());
                    PolicyDecision::Deny {
                        reason: format!(
                            "medium-risk tool '{}' requires an Active+Claimed task (current: {})",
                            tool, state_desc
                        ),
                    }
                }
            }

            RiskLevel::High | RiskLevel::Critical => PolicyDecision::NeedsApproval {
                tool: tool.to_string(),
                risk,
                reason: format!(
                    "tool '{}' requires explicit approval before execution",
                    tool
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> ApprovalRules {
        ApprovalRules::default()
    }

    #[test]
    fn low_risk_allows() {
        let r = rules();
        assert_eq!(
            r.should_approve("read_file", RiskLevel::Low, None),
            PolicyDecision::Allow
        );
    }

    #[test]
    fn medium_without_task_denies() {
        let r = rules();
        let decision = r.should_approve("run_tests", RiskLevel::Medium, None);
        matches!(decision, PolicyDecision::Deny { .. });
    }

    #[test]
    fn medium_with_active_task_allows() {
        let r = rules();
        let state = TaskState::Active;
        assert_eq!(
            r.should_approve("run_tests", RiskLevel::Medium, Some(&state)),
            PolicyDecision::Allow
        );
    }

    #[test]
    fn medium_with_pending_task_denies() {
        let r = rules();
        let state = TaskState::Pending;
        let decision = r.should_approve("run_tests", RiskLevel::Medium, Some(&state));
        assert!(matches!(decision, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn high_risk_needs_approval() {
        let r = rules();
        let decision = r.should_approve("apply_patch", RiskLevel::High, Some(&TaskState::Active));
        assert!(matches!(decision, PolicyDecision::NeedsApproval { .. }));
    }

    #[test]
    fn critical_risk_needs_approval() {
        let r = rules();
        let decision = r.should_approve("git_push", RiskLevel::Critical, None);
        assert!(matches!(decision, PolicyDecision::NeedsApproval { .. }));
    }
}
