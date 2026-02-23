//! Definition-of-Done (DoD) checker.
//!
//! Called when an agent attempts to transition a task to `CodeReview`.  The
//! checker ensures:
//! 1. The task spec has acceptance criteria.
//! 2. The patch being submitted has no placeholder stubs (TODO, FIXME, etc.).
//! 3. Tests were run.
//! 4. Tests are passing.

use crate::tasks::schema::TaskSpec;

use super::sandbox::PolicyViolation;
use super::scanners::check_no_placeholders;

// ─── DoD checker ─────────────────────────────────────────────────────────────

/// Stateless DoD validator.
pub struct DodChecker;

impl DodChecker {
    /// Check that a task is ready to transition to `CodeReview`.
    ///
    /// Returns a list of violations. An empty list means the task passes all
    /// DoD gates. Callers should block the transition if any violations are
    /// returned.
    ///
    /// # Arguments
    ///
    /// * `task_spec`     — The task spec for the task being transitioned.
    /// * `test_ran`      — Whether tests have been run for this task.
    /// * `tests_passing` — Whether the last test run passed. Ignored if `test_ran` is false.
    /// * `patch_content` — The cumulative unified diff that will be code-reviewed.
    pub fn check_transition_to_cr(
        task_spec: &TaskSpec,
        test_ran: bool,
        tests_passing: bool,
        patch_content: &str,
    ) -> Vec<PolicyViolation> {
        let mut violations = Vec::new();

        // ── Gate 1: acceptance criteria must be present ───────────────────
        if task_spec.acceptance_criteria.is_empty() {
            violations.push(PolicyViolation::NoAcceptanceCriteria);
        }

        // ── Gate 2: no placeholder stubs in the patch ─────────────────────
        if let Err(v) = check_no_placeholders(patch_content) {
            violations.push(v);
        }

        // ── Gate 3: tests must have been run ──────────────────────────────
        if !test_ran {
            violations.push(PolicyViolation::TestsNotRun);
        } else if !tests_passing {
            // ── Gate 4: tests must be passing ──────────────────────────────
            violations.push(PolicyViolation::TestsFailing);
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::schema::{Priority, RiskLevel, TaskSpec};
    use chrono::Utc;

    fn spec_with_criteria(criteria: Vec<&str>) -> TaskSpec {
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

    const CLEAN_PATCH: &str = "\
--- a/main.rs
+++ b/main.rs
@@ -1,1 +1,2 @@
 fn main() {}
+fn new_fn() { println!(\"hello\"); }
";

    const PLACEHOLDER_PATCH: &str = "\
--- a/main.rs
+++ b/main.rs
@@ -1,1 +1,3 @@
 fn main() {}
+fn new_fn() {
+    // TODO: implement this
+}
";

    #[test]
    fn all_clear_no_violations() {
        let spec = spec_with_criteria(vec!["feature works end to end"]);
        let violations =
            DodChecker::check_transition_to_cr(&spec, true, true, CLEAN_PATCH);
        assert!(violations.is_empty(), "violations: {:?}", violations);
    }

    #[test]
    fn no_acceptance_criteria_violation() {
        let spec = spec_with_criteria(vec![]);
        let violations =
            DodChecker::check_transition_to_cr(&spec, true, true, CLEAN_PATCH);
        assert!(violations
            .iter()
            .any(|v| matches!(v, PolicyViolation::NoAcceptanceCriteria)));
    }

    #[test]
    fn placeholder_in_patch_violation() {
        let spec = spec_with_criteria(vec!["feature works"]);
        let violations =
            DodChecker::check_transition_to_cr(&spec, true, true, PLACEHOLDER_PATCH);
        assert!(violations
            .iter()
            .any(|v| matches!(v, PolicyViolation::PlaceholderDetected { .. })));
    }

    #[test]
    fn tests_not_run_violation() {
        let spec = spec_with_criteria(vec!["feature works"]);
        let violations =
            DodChecker::check_transition_to_cr(&spec, false, false, CLEAN_PATCH);
        assert!(violations
            .iter()
            .any(|v| matches!(v, PolicyViolation::TestsNotRun)));
    }

    #[test]
    fn tests_failing_violation() {
        let spec = spec_with_criteria(vec!["feature works"]);
        let violations =
            DodChecker::check_transition_to_cr(&spec, true, false, CLEAN_PATCH);
        assert!(violations
            .iter()
            .any(|v| matches!(v, PolicyViolation::TestsFailing)));
    }
}
