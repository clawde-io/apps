// SPDX-License-Identifier: MIT
//! AI synthesis layer — Sprint O (CR.T14–CR.T16)
//!
//! Groups review findings by theme and synthesises coherent review comments.

use crate::code_review::model::{Grade, ReviewComment, ReviewIssue, ReviewSeverity};

/// Group a flat list of issues into themed [`ReviewComment`]s.
pub fn synthesise(issues: &[ReviewIssue]) -> Vec<ReviewComment> {
    if issues.is_empty() {
        return Vec::new();
    }

    // Group by (file, code) — simple grouping heuristic.
    let mut map: std::collections::HashMap<String, Vec<&ReviewIssue>> =
        std::collections::HashMap::new();
    for issue in issues {
        let key = format!(
            "{}:{}",
            issue.file,
            issue.code.as_deref().unwrap_or("general")
        );
        map.entry(key).or_default().push(issue);
    }

    map.into_values()
        .map(|group| {
            let first = group[0];
            ReviewComment {
                file: Some(first.file.clone()),
                theme: first.code.clone().unwrap_or_else(|| "general".to_string()),
                severity: first.severity,
                explanation: first.message.clone(),
                suggestions: group
                    .iter()
                    .filter_map(|i| i.fix_suggestion.clone())
                    .collect(),
                is_uncertain: false,
            }
        })
        .collect()
}

/// Compute an overall grade from a list of issues.
pub fn grade_from_issues(issues: &[ReviewIssue]) -> Grade {
    let errors = issues
        .iter()
        .filter(|i| i.severity == ReviewSeverity::Error)
        .count();
    let warnings = issues
        .iter()
        .filter(|i| i.severity == ReviewSeverity::Warning)
        .count();
    Grade::from_counts(errors, warnings)
}
