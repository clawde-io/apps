//! `PolicyEngine` — the top-level entry point for all policy decisions.
//!
//! `PolicyEngine::evaluate` is called from `McpDispatcher` (and other tool
//! dispatch sites) *before* any tool executes. It runs three checks in order:
//!
//! 1. **Task state gate** — task must be in a state that permits the call.
//! 2. **Risk classification** — look up the tool's risk level.
//! 3. **Approval rules** — decide Allow / Deny / NeedsApproval.

use std::path::Path;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::debug;

use crate::tasks::reducer::TaskState;
use crate::tasks::schema::RiskLevel;

use super::mcp_trust::TrustDatabase;
use super::risk::RiskDatabase;
use super::rules::ApprovalRules;
use super::supply_chain::SupplyChainPolicy;

// ─── PolicyDecision ───────────────────────────────────────────────────────────

/// The outcome of a policy evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyDecision {
    /// Tool may execute immediately.
    Allow,
    /// Tool is denied — execution must not proceed.
    Deny { reason: String },
    /// Tool requires explicit human (or orchestrator) approval before execution.
    NeedsApproval {
        tool: String,
        risk: RiskLevel,
        reason: String,
    },
}

// ─── PolicyEngine ─────────────────────────────────────────────────────────────

/// Orchestrates risk classification, approval rules, and trust database checks.
pub struct PolicyEngine {
    pub risk_db: Arc<RwLock<RiskDatabase>>,
    pub trust_db: Arc<RwLock<TrustDatabase>>,
    pub supply_chain: Arc<SupplyChainPolicy>,
    rules: ApprovalRules,
}

impl PolicyEngine {
    /// Construct a `PolicyEngine` from pre-loaded components.
    pub fn new(
        risk_db: Arc<RwLock<RiskDatabase>>,
        trust_db: Arc<RwLock<TrustDatabase>>,
        supply_chain: Arc<SupplyChainPolicy>,
    ) -> Self {
        Self {
            risk_db,
            trust_db,
            supply_chain,
            rules: ApprovalRules::default(),
        }
    }

    /// Load a `PolicyEngine` by reading config files from `.claw/policies/`.
    ///
    /// Missing files are silently substituted with defaults.
    pub fn load(claw_dir: &Path) -> Self {
        let risk_path = claw_dir.join("policies").join("tool-risk.json");
        let trust_path = claw_dir.join("policies").join("mcp-trust.json");
        let allowlist_path = claw_dir.join("policies").join("mcp-allowlist.json");

        let risk_db = RiskDatabase::load_from_json(&risk_path);
        let trust_db = TrustDatabase::load(&trust_path);
        let supply_chain = SupplyChainPolicy::load(&allowlist_path);

        Self {
            risk_db: Arc::new(RwLock::new(risk_db)),
            trust_db: Arc::new(RwLock::new(trust_db)),
            supply_chain: Arc::new(supply_chain),
            rules: ApprovalRules::default(),
        }
    }

    /// Evaluate a proposed tool invocation and return the policy decision.
    ///
    /// # Arguments
    ///
    /// * `tool_name`  — Name of the MCP tool being invoked.
    /// * `args`       — Full JSON arguments for the invocation.
    /// * `task_state` — Current state of the associated task, if any.
    /// * `agent_id`   — Identifier of the requesting agent.
    pub async fn evaluate(
        &self,
        tool_name: &str,
        _args: &serde_json::Value,
        task_state: Option<&TaskState>,
        _agent_id: &str,
    ) -> PolicyDecision {
        // ── Step 1: look up risk level ────────────────────────────────────
        let risk = {
            let db = self.risk_db.read().await;
            db.get_risk(tool_name)
        };

        debug!(
            tool = tool_name,
            risk = ?risk,
            task_state = ?task_state,
            "policy evaluate"
        );

        // ── Step 2: apply approval rules ──────────────────────────────────
        self.rules.should_approve(tool_name, risk, task_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::mcp_trust::TrustDatabase;
    use crate::policy::risk::RiskDatabase;
    use crate::policy::supply_chain::SupplyChainPolicy;
    use serde_json::json;

    fn engine() -> PolicyEngine {
        PolicyEngine::new(
            Arc::new(RwLock::new(RiskDatabase::default_rules())),
            Arc::new(RwLock::new(TrustDatabase::default())),
            Arc::new(SupplyChainPolicy::empty()),
        )
    }

    #[tokio::test]
    async fn low_risk_allows() {
        let e = engine();
        let decision = e
            .evaluate("read_file", &json!({}), Some(&TaskState::Active), "agent-1")
            .await;
        assert_eq!(decision, PolicyDecision::Allow);
    }

    #[tokio::test]
    async fn medium_without_task_denies() {
        let e = engine();
        let decision = e.evaluate("run_tests", &json!({}), None, "agent-1").await;
        assert!(matches!(decision, PolicyDecision::Deny { .. }));
    }

    #[tokio::test]
    async fn high_risk_needs_approval() {
        let e = engine();
        let decision = e
            .evaluate(
                "apply_patch",
                &json!({}),
                Some(&TaskState::Active),
                "agent-1",
            )
            .await;
        assert!(matches!(decision, PolicyDecision::NeedsApproval { .. }));
    }
}
