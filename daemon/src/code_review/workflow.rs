// SPDX-License-Identifier: MIT
//! Review workflow orchestrator — Sprint O (CR.T17–CR.T18)
//!
//! Drives the full review pipeline: run tools → build codegraph →
//! synthesise comments → compute grade → return [`ReviewResult`].

use crate::code_review::{
    ai_synthesis, codegraph,
    model::{Grade, ReviewConfig, ReviewResult, ToolResult},
    tool_runner::ToolRunner,
};
use anyhow::Result;
use std::path::Path;
use uuid::Uuid;

/// Run a full review for the repo at `repo_path` using `config`.
pub async fn run_review(repo_path: &Path, config: &ReviewConfig) -> Result<ReviewResult> {
    let id = Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();

    // 1. Run each enabled tool and collect results.
    let mut tool_results: Vec<ToolResult> = Vec::new();
    let mut all_issues = Vec::new();

    for tool_cfg in &config.tools {
        if !tool_cfg.enabled {
            continue;
        }
        let (issues, result) = ToolRunner::run_tool(tool_cfg, repo_path, None).await?;
        all_issues.extend(issues);
        tool_results.push(result);
    }

    // 2. Run codegraph to detect breaking changes (best-effort).
    let diff_text = String::new(); // Future: pass in actual diff
    let fns = codegraph::changed_functions(&diff_text);
    if let Ok(breaking) = codegraph::detect_breaking_changes(&fns) {
        all_issues.extend(breaking);
    }

    // 3. Filter by severity threshold.
    all_issues.retain(|i| i.severity >= config.severity_threshold);

    // 4. Synthesise AI comments.
    let comments = ai_synthesis::synthesise(&all_issues);
    let grade = ai_synthesis::grade_from_issues(&all_issues);

    // 5. Count errors/warnings.
    let error_count = all_issues
        .iter()
        .filter(|i| i.severity == crate::code_review::model::ReviewSeverity::Error)
        .count();
    let warning_count = all_issues
        .iter()
        .filter(|i| i.severity == crate::code_review::model::ReviewSeverity::Warning)
        .count();

    // 6. Check grade requirement.
    if let Some(required) = config.require_grade {
        if !grade.meets(required) {
            tracing::warn!(
                "Review grade {:?} does not meet required {:?}",
                grade,
                required
            );
        }
    }

    Ok(ReviewResult {
        id,
        grade,
        summary: build_summary(&grade, error_count, warning_count),
        comments,
        tool_results,
        created_at,
        total_issues: all_issues.len(),
        error_count,
        warning_count,
    })
}

fn build_summary(grade: &Grade, errors: usize, warnings: usize) -> String {
    format!(
        "Grade {grade}: {errors} error(s), {warnings} warning(s) found."
    )
}
