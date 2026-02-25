// SPDX-License-Identifier: MIT
//! Drift enforcer — AE.T04–T07 (Autonomous Execution Engine, Sprint J).
//!
//! Compares the set of files the AI actually touched against the `files_expected`
//! list in the active `AePlan` and classifies any divergence as:
//!
//! - **Cosmetic drift** — files outside the expected set that are clearly
//!   incidental (docs, lock files, formatting-only changes).  Logged only.
//! - **Structural drift** — expected files not touched, or unexpected source
//!   files modified.  Emits a `session.driftWarning` push event.
//! - **Plan drift** — the set of changes would fundamentally alter the scope
//!   (e.g. a new database table added when only UI was planned).  Blocks
//!   execution; requires user re-approval.
//!
//! Also provides `inject_plan_reminder` to construct the periodic context
//! re-injection message (AE.T04).

use std::path::{Path, PathBuf};

use crate::autonomous::AePlan;

// ─── DriftResult ─────────────────────────────────────────────────────────────

/// Result of comparing expected vs. actual file sets.
#[derive(Debug, Clone)]
pub struct DriftResult {
    /// Files touched outside the expected set that look cosmetic (docs, lock).
    pub cosmetic: Vec<PathBuf>,
    /// Human-readable descriptions of structural divergences.
    pub structural: Vec<String>,
    /// `true` when the drift is severe enough to require plan re-approval.
    pub plan_drift: bool,
}

impl DriftResult {
    /// `true` when there is no drift at all.
    pub fn is_clean(&self) -> bool {
        self.cosmetic.is_empty() && self.structural.is_empty() && !self.plan_drift
    }
}

// ─── DriftEnforcer ───────────────────────────────────────────────────────────

pub struct DriftEnforcer;

impl DriftEnforcer {
    /// Compare expected vs. actual file paths and classify any drift.
    ///
    /// `expected_files` comes from `AePlan.files_expected`.
    /// `actual_files`   comes from the git diff of the session's working tree.
    pub fn check_file_drift(expected_files: &[PathBuf], actual_files: &[PathBuf]) -> DriftResult {
        let mut cosmetic: Vec<PathBuf> = Vec::new();
        let mut structural: Vec<String> = Vec::new();

        // ── Files touched but not expected ────────────────────────────────────
        for actual in actual_files {
            let in_expected = expected_files.iter().any(|e| paths_overlap(e, actual));
            if !in_expected {
                if is_cosmetic_path(actual) {
                    cosmetic.push(actual.clone());
                } else {
                    structural.push(format!("Unexpected file modified: {}", actual.display()));
                }
            }
        }

        // ── Expected files not touched ─────────────────────────────────────
        for expected in expected_files {
            let was_touched = actual_files.iter().any(|a| paths_overlap(expected, a));
            if !was_touched {
                structural.push(format!(
                    "Expected file not modified: {}",
                    expected.display()
                ));
            }
        }

        // Plan drift: triggered when unexpected schema/migration files appear.
        let plan_drift = actual_files
            .iter()
            .any(|f| is_schema_path(f) && !expected_files.iter().any(|e| paths_overlap(e, f)));

        DriftResult {
            cosmetic,
            structural,
            plan_drift,
        }
    }

    /// Build the periodic plan-reminder message injected every 10 turns (AE.T04).
    ///
    /// The returned string is inserted as a system message before the next
    /// assistant turn to prevent context drift.
    pub fn inject_plan_reminder(plan: &AePlan) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "PLAN REMINDER — you are working on: {}",
            plan.title
        ));

        if !plan.definition_of_done.is_empty() {
            lines.push("Definition of done:".to_owned());
            for item in &plan.definition_of_done {
                lines.push(format!("  - {item}"));
            }
        }

        if !plan.files_expected.is_empty() {
            let paths: Vec<String> = plan
                .files_expected
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            lines.push(format!("Expected files: {}", paths.join(", ")));
        }

        lines.push(
            "Do not modify files outside the expected set without flagging it first.".to_owned(),
        );

        lines.join("\n")
    }

    /// Build the structural-drift correction message (AE.T07).
    ///
    /// Injected when structural drift is detected to redirect the AI back to
    /// the plan without blocking execution.
    pub fn build_correction_message(drift: &DriftResult, plan: &AePlan) -> String {
        let issues: Vec<&str> = drift.structural.iter().map(String::as_str).collect();
        format!(
            "DRIFT DETECTED — you are drifting from the plan \"{}\".\n\
             Issues:\n{}\n\
             Please return to the planned scope. Only modify files listed in the plan.",
            plan.title,
            issues.join("\n  - ")
        )
    }
}

// ─── Path helpers ─────────────────────────────────────────────────────────────

/// Returns `true` if `expected` and `actual` refer to the same file
/// (allowing for prefix/suffix variations, e.g. relative vs. absolute).
fn paths_overlap(expected: &Path, actual: &Path) -> bool {
    // Exact match first.
    if expected == actual {
        return true;
    }
    // Compare by file name only when one side has no parent.
    let e_name = expected.file_name();
    let a_name = actual.file_name();
    if e_name.is_some() && e_name == a_name {
        return true;
    }
    // Suffix match: actual ends with expected (or vice versa).
    let e_str = expected.to_string_lossy();
    let a_str = actual.to_string_lossy();
    a_str.ends_with(e_str.as_ref()) || e_str.ends_with(a_str.as_ref())
}

/// Paths that are considered cosmetic (safe to touch without triggering drift).
fn is_cosmetic_path(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    matches!(
        name.as_str(),
        "cargo.lock"
            | "pubspec.lock"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "readme.md"
            | "changelog.md"
            | ".gitignore"
            | ".gitattributes"
            | "rustfmt.toml"
            | ".editorconfig"
    ) || matches!(ext.as_str(), "lock" | "sum")
}

/// Returns `true` for schema or migration files (triggers plan-drift check).
fn is_schema_path(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    name.ends_with(".sql")
        || name.contains("migration")
        || name.contains("schema")
        || name.contains("migrate")
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn pb(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn test_clean_when_files_match() {
        let expected = vec![pb("src/main.rs"), pb("src/lib.rs")];
        let actual = vec![pb("src/main.rs"), pb("src/lib.rs")];
        let result = DriftEnforcer::check_file_drift(&expected, &actual);
        assert!(result.is_clean());
    }

    #[test]
    fn test_cosmetic_drift_lock_file() {
        let expected = vec![pb("src/main.rs")];
        let actual = vec![pb("src/main.rs"), pb("Cargo.lock")];
        let result = DriftEnforcer::check_file_drift(&expected, &actual);
        assert_eq!(result.cosmetic.len(), 1);
        assert!(result.structural.is_empty());
        assert!(!result.plan_drift);
    }

    #[test]
    fn test_structural_drift_unexpected_source() {
        let expected = vec![pb("desktop/lib/main.dart")];
        let actual = vec![pb("desktop/lib/main.dart"), pb("daemon/src/session.rs")];
        let result = DriftEnforcer::check_file_drift(&expected, &actual);
        assert!(!result.structural.is_empty());
        assert!(!result.plan_drift);
    }

    #[test]
    fn test_plan_drift_unexpected_migration() {
        let expected = vec![pb("src/handlers.rs")];
        let actual = vec![pb("src/handlers.rs"), pb("migrations/020_new_table.sql")];
        let result = DriftEnforcer::check_file_drift(&expected, &actual);
        assert!(result.plan_drift);
    }

    #[test]
    fn test_inject_plan_reminder_contains_title() {
        let plan = crate::autonomous::PlanGenerator::generate_plan(
            "Add OAuth middleware.\n- Must validate JWT",
            "sess-test",
        )
        .unwrap();
        let reminder = DriftEnforcer::inject_plan_reminder(&plan);
        assert!(reminder.contains(&plan.title));
    }

    #[test]
    fn test_missing_expected_file_is_structural() {
        let expected = vec![pb("src/auth.rs"), pb("src/lib.rs")];
        let actual = vec![pb("src/lib.rs")];
        let result = DriftEnforcer::check_file_drift(&expected, &actual);
        assert!(!result.structural.is_empty());
        let has_missing = result.structural.iter().any(|s| s.contains("src/auth.rs"));
        assert!(has_missing, "Expected structural message about src/auth.rs");
    }
}
