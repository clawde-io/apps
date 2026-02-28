//! Integration tests for the Policy Engine (Phase 43d).
//!
//! Tests cover:
//! 1. Low-risk tool auto-allow
//! 2. Medium-risk tool denied without active task
//! 3. High-risk tool returns NeedsApproval
//! 4. Path escape blocked
//! 5. Secrets in patch blocked
//! 6. Placeholder DoD check
//! 7. Planner RBAC cannot apply_patch

use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use tokio::sync::RwLock;

use clawd::policy::approval::ApprovalRouter;
use clawd::policy::dod::DodChecker;
use clawd::policy::mcp_trust::TrustDatabase;
use clawd::policy::output_scan::scan_patch_output;
use clawd::policy::rbac::{check_tool_authorized, AgentRole};
use clawd::policy::risk::RiskDatabase;
use clawd::policy::sandbox::SandboxPolicy;
use clawd::policy::supply_chain::SupplyChainPolicy;
use clawd::policy::{PolicyDecision, PolicyEngine, PolicyViolation};
use clawd::tasks::reducer::TaskState;
use clawd::tasks::schema::{Priority, RiskLevel, TaskSpec};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn make_engine() -> PolicyEngine {
    PolicyEngine::new(
        Arc::new(RwLock::new(RiskDatabase::default_rules())),
        Arc::new(RwLock::new(TrustDatabase::default())),
        Arc::new(SupplyChainPolicy::empty()),
    )
}

fn make_spec(criteria: Vec<&str>) -> TaskSpec {
    use chrono::Utc;
    TaskSpec {
        id: "t1".to_string(),
        title: "Test task".to_string(),
        repo: "/tmp".to_string(),
        summary: None,
        acceptance_criteria: criteria.into_iter().map(String::from).collect(),
        test_plan: None,
        risk_level: RiskLevel::Medium,
        priority: Priority::Medium,
        labels: vec![],
        owner: None,
        worktree_path: None,
        worktree_branch: None,
        created_at: Utc::now(),
    }
}

// ─── Test 1: low-risk tool auto-allow ────────────────────────────────────────

#[tokio::test]
async fn test_low_risk_auto_allow() {
    let engine = make_engine();

    // read_file is Low risk — should always Allow regardless of task state.
    let decision = engine
        .evaluate("read_file", &json!({}), None, "agent-1")
        .await;

    assert_eq!(
        decision,
        PolicyDecision::Allow,
        "expected Allow for low-risk tool"
    );
}

// ─── Test 2: medium-risk tool denied without active task ─────────────────────

#[tokio::test]
async fn test_medium_requires_active_task() {
    let engine = make_engine();

    // run_tests is Medium risk — requires Active task state.
    let decision_no_task = engine
        .evaluate("run_tests", &json!({}), None, "agent-1")
        .await;

    assert!(
        matches!(decision_no_task, PolicyDecision::Deny { .. }),
        "expected Deny without task; got {:?}",
        decision_no_task
    );

    // With a Pending task — still denied.
    let decision_pending = engine
        .evaluate(
            "run_tests",
            &json!({}),
            Some(&TaskState::Pending),
            "agent-1",
        )
        .await;

    assert!(
        matches!(decision_pending, PolicyDecision::Deny { .. }),
        "expected Deny for Pending task; got {:?}",
        decision_pending
    );

    // With an Active task — allowed.
    let decision_active = engine
        .evaluate("run_tests", &json!({}), Some(&TaskState::Active), "agent-1")
        .await;

    assert_eq!(
        decision_active,
        PolicyDecision::Allow,
        "expected Allow for Active task"
    );
}

// ─── Test 3: high-risk tool returns NeedsApproval ────────────────────────────

#[tokio::test]
async fn test_high_risk_needs_approval() {
    let engine = make_engine();

    let decision = engine
        .evaluate(
            "apply_patch",
            &json!({}),
            Some(&TaskState::Active),
            "agent-1",
        )
        .await;

    assert!(
        matches!(decision, PolicyDecision::NeedsApproval { .. }),
        "expected NeedsApproval for apply_patch; got {:?}",
        decision
    );

    if let PolicyDecision::NeedsApproval { tool, risk, .. } = decision {
        assert_eq!(tool, "apply_patch");
        assert_eq!(risk, RiskLevel::High);
    }
}

// ─── Test 4: path outside worktree returns PolicyViolation ───────────────────

#[tokio::test]
async fn test_path_escape_blocked() {
    let sandbox = SandboxPolicy::new("/tmp/task-worktree", false);

    // Path inside worktree — OK.
    let inside = Path::new("/tmp/task-worktree/src/main.rs");
    assert!(
        sandbox.check_path(inside).is_ok(),
        "inside-worktree path should be allowed"
    );

    // Path outside worktree — violation.
    let outside = Path::new("/etc/passwd");
    let result = sandbox.check_path(outside);
    assert!(
        matches!(result, Err(PolicyViolation::PathEscape { .. })),
        "expected PathEscape; got {:?}",
        result
    );
}

// ─── Test 5: secrets in patch caught by output scan ──────────────────────────

#[tokio::test]
async fn test_secrets_in_patch_blocked() {
    let patch_with_key = "\
--- a/config.rs
+++ b/config.rs
@@ -1,1 +1,2 @@
 fn config() {}
+const API_KEY: &str = \"sk-abcdefghijklmnopqrstuvwxyz1234567890\";
";

    let violations = scan_patch_output(patch_with_key);

    assert!(
        !violations.is_empty(),
        "expected at least one secret violation"
    );

    assert!(
        violations
            .iter()
            .any(|v| matches!(v, PolicyViolation::SecretDetected { .. })),
        "expected SecretDetected violation"
    );
}

// ─── Test 6: placeholder in patch blocked on CR transition ───────────────────

#[tokio::test]
async fn test_placeholder_dod() {
    let spec = make_spec(vec!["feature works end to end"]);

    let patch_with_todo = "\
--- a/main.rs
+++ b/main.rs
@@ -1,1 +1,3 @@
 fn main() {}
+fn new_fn() {
+    // TODO: implement this
+}
";

    let violations = DodChecker::check_transition_to_cr(&spec, true, true, patch_with_todo);

    assert!(
        violations
            .iter()
            .any(|v| matches!(v, PolicyViolation::PlaceholderDetected { .. })),
        "expected PlaceholderDetected violation; got {:?}",
        violations
    );
}

// ─── Test 7: planner RBAC cannot apply_patch ─────────────────────────────────

#[tokio::test]
async fn test_rbac_planner_cannot_patch() {
    let result = check_tool_authorized(&AgentRole::Planner, "apply_patch");

    assert!(
        matches!(result, Err(PolicyViolation::UnauthorizedTool { .. })),
        "expected UnauthorizedTool for Planner + apply_patch; got {:?}",
        result
    );

    if let Err(PolicyViolation::UnauthorizedTool { tool, role }) = result {
        assert_eq!(tool, "apply_patch");
        assert_eq!(role, "planner");
    }
}

// ─── Bonus: approval router round-trip ───────────────────────────────────────

#[tokio::test]
async fn test_approval_router_grant_deny() {
    let router = ApprovalRouter::new();

    let id = router
        .request_approval(
            "t1",
            "agent-1",
            "apply_patch",
            "apply big patch",
            RiskLevel::High,
        )
        .await;

    // Verify pending.
    let req = router.get(&id).await.expect("request should exist");
    assert_eq!(req.status, clawd::policy::approval::ApprovalStatus::Pending);

    // Grant.
    router.grant(&id).await.expect("grant should succeed");

    let req = router.get(&id).await.expect("request should exist");
    assert_eq!(req.status, clawd::policy::approval::ApprovalStatus::Granted);
}

// ─── Bonus: DoD all gates pass ────────────────────────────────────────────────

#[tokio::test]
async fn test_dod_all_gates_pass() {
    let spec = make_spec(vec!["acceptance criterion met"]);

    let clean_patch = "\
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,1 +1,2 @@
 pub fn existing() {}
+pub fn new_feature() -> bool { true }
";

    let violations = DodChecker::check_transition_to_cr(&spec, true, true, clean_patch);

    assert!(
        violations.is_empty(),
        "expected no DoD violations; got {:?}",
        violations
    );
}
