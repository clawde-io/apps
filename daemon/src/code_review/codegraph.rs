// SPDX-License-Identifier: MIT
//! Codegraph builder — Sprint O (CR.T08–CR.T10)
//!
//! Parses git diffs to detect changed functions and identify breaking changes.

use crate::code_review::model::{ReviewIssue, ReviewSeverity};
use anyhow::Result;

/// Summarise which functions/methods were changed in `diff_text`.
pub fn changed_functions(diff_text: &str) -> Vec<String> {
    let mut functions = Vec::new();
    for line in diff_text.lines() {
        // Heuristic: look for `@@` lines with function context
        if let Some(rest) = line.strip_prefix("@@ ") {
            if let Some(ctx) = rest.split("@@").nth(1) {
                let ctx = ctx.trim();
                if !ctx.is_empty() {
                    functions.push(ctx.to_string());
                }
            }
        }
    }
    functions
}

/// Detect potential breaking-change issues from a set of changed function names.
pub fn detect_breaking_changes(functions: &[String]) -> Result<Vec<ReviewIssue>> {
    let mut issues = Vec::new();
    for f in functions {
        // Simple heuristic: public API changes on functions with `pub` context are flagged.
        if f.contains("pub ") {
            issues.push(ReviewIssue {
                file: String::new(),
                line: 0,
                col: None,
                severity: ReviewSeverity::Warning,
                tool: "codegraph".to_string(),
                message: format!("Possible public API change in: {f}"),
                fix_suggestion: None,
                code: Some("breaking-change".to_string()),
            });
        }
    }
    Ok(issues)
}
