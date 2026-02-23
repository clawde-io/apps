//! Eval runner — loads fixtures and runs them through the scanner suite.
//!
//! Fixtures live in `.claw/evals/datasets/*.json`.  Each fixture describes a
//! tool call or patch, the expected outcome, and which violation types should
//! be detected.  The runner compares actual results against expectations and
//! produces `EvalResult` records for the report generator.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::evals::scanners::{
    forbidden::check_tool_allowed,
    placeholders::scan_patch as placeholder_scan,
    secrets::scan_patch as secret_scan,
};

// ─── Fixture types ────────────────────────────────────────────────────────────

/// A single eval fixture describing one scenario to validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalFixture {
    /// Human-readable name for reporting.
    pub name: String,
    /// Input payload — a tool call description or unified diff.
    pub input: serde_json::Value,
    /// Expected outcome: `"allowed"`, `"blocked"`, or `"redacted"`.
    pub expected_outcome: String,
    /// Violation type strings expected to be present (e.g. `["secret"]`).
    pub expected_violations: Vec<String>,
}

// ─── Result types ─────────────────────────────────────────────────────────────

/// Outcome of running a single fixture through the scanner suite.
#[derive(Debug)]
pub struct EvalResult {
    /// Name of the fixture that produced this result.
    pub fixture: String,
    /// Whether the actual outcome matched the expected outcome.
    pub passed: bool,
    /// The outcome determined by the scanner suite.
    pub actual_outcome: String,
    /// Violation types that were actually found.
    pub violations_found: Vec<String>,
    /// Human-readable description of each difference from expected.
    pub diffs: Vec<String>,
}

// ─── Loading ─────────────────────────────────────────────────────────────────

/// Load all fixture files from `.claw/evals/datasets/`.
///
/// Each file must be a JSON array of `EvalFixture` objects.
pub async fn load_fixtures(data_dir: &Path) -> Result<Vec<EvalFixture>> {
    let datasets_dir = data_dir.join("evals").join("datasets");

    if !datasets_dir.exists() {
        return Ok(Vec::new());
    }

    let mut fixtures = Vec::new();
    let mut entries = tokio::fs::read_dir(&datasets_dir)
        .await
        .with_context(|| format!("read datasets dir: {}", datasets_dir.display()))?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("read fixture file: {}", path.display()))?;
        let file_fixtures: Vec<EvalFixture> = serde_json::from_str(&content)
            .with_context(|| format!("parse fixture file: {}", path.display()))?;
        debug!(file = %path.display(), count = file_fixtures.len(), "loaded eval fixtures");
        fixtures.extend(file_fixtures);
    }

    Ok(fixtures)
}

// ─── Execution ───────────────────────────────────────────────────────────────

/// Run all fixtures through the scanner suite and return results.
pub async fn run_evals(fixtures: &[EvalFixture]) -> Vec<EvalResult> {
    let mut results = Vec::with_capacity(fixtures.len());

    for fixture in fixtures {
        let result = run_one(fixture).await;
        results.push(result);
    }

    results
}

// ─── Private ─────────────────────────────────────────────────────────────────

async fn run_one(fixture: &EvalFixture) -> EvalResult {
    let mut violations_found: Vec<String> = Vec::new();

    // Extract patch/content and tool_name from input.
    let patch = fixture
        .input
        .get("patch")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let tool_name = fixture
        .input
        .get("tool")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let permissions: Vec<String> = fixture
        .input
        .get("permissions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let target_path = fixture
        .input
        .get("target_path")
        .and_then(|v| v.as_str());
    let worktree_path = fixture
        .input
        .get("worktree_path")
        .and_then(|v| v.as_str());

    // Run placeholder scanner.
    if !patch.is_empty() {
        let ph = placeholder_scan(patch);
        if !ph.is_empty() {
            violations_found.push("placeholder".to_string());
        }

        // Run secrets scanner.
        let sec = secret_scan(patch);
        if !sec.is_empty() {
            violations_found.push("secret".to_string());
        }
    }

    // Run forbidden tool scanner.
    if !tool_name.is_empty() {
        if let Some(_v) = check_tool_allowed(tool_name, &permissions, target_path, worktree_path) {
            violations_found.push("forbidden_tool".to_string());
        }
    }

    // Determine actual outcome.
    let actual_outcome = if violations_found.is_empty() {
        "allowed".to_string()
    } else if violations_found.contains(&"secret".to_string()) {
        "redacted".to_string()
    } else {
        "blocked".to_string()
    };

    // Compare against expectations.
    let mut diffs = Vec::new();
    if actual_outcome != fixture.expected_outcome {
        diffs.push(format!(
            "outcome: expected `{}`, got `{}`",
            fixture.expected_outcome, actual_outcome
        ));
    }
    for expected_v in &fixture.expected_violations {
        if !violations_found.contains(expected_v) {
            diffs.push(format!("missing expected violation: `{}`", expected_v));
        }
    }
    for found_v in &violations_found {
        if !fixture.expected_violations.contains(found_v) {
            diffs.push(format!("unexpected violation found: `{}`", found_v));
        }
    }

    EvalResult {
        fixture: fixture.name.clone(),
        passed: diffs.is_empty(),
        actual_outcome,
        violations_found,
        diffs,
    }
}
