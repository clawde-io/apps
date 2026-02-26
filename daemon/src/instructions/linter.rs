// instructions/linter.rs — Instruction linting: conflict + ambiguity detection (Sprint ZZ IL.T01/T02)
//
// `clawd instructions lint` runs:
//   1. ConflictDetector — semantic contradictions (npm vs pnpm, etc.)
//   2. AmbiguityLinter  — vague language ("properly", "as needed", "best effort")
//   3. Budget check     — bytes vs target limit

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LintSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintIssue {
    pub severity: LintSeverity,
    pub rule: String,
    pub message: String,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LintReport {
    pub errors: Vec<LintIssue>,
    pub warnings: Vec<LintIssue>,
    pub passed: bool,
}

/// Vague instruction words that make rules non-testable
const AMBIGUOUS_WORDS: &[&str] = &[
    "properly", "as needed", "best effort", "ideally", "when possible",
    "appropriately", "reasonable", "should consider", "try to", "might want",
    "feel free", "in most cases", "generally speaking",
];

/// Package manager contradiction pairs
const PM_PAIRS: &[(&str, &str, &str)] = &[
    ("npm", "pnpm", "package-manager"),
    ("npm", "yarn", "package-manager"),
    ("yarn", "pnpm", "package-manager"),
];

/// Logging contradiction pairs
const LOG_PAIRS: &[(&str, &str, &str)] = &[
    ("never use console.log", "always log to console", "logging"),
    ("no console.log", "use console.log", "logging"),
];

pub fn lint_nodes(nodes: &[LintNode], budget_bytes: usize) -> LintReport {
    let mut errors: Vec<LintIssue>   = Vec::new();
    let mut warnings: Vec<LintIssue> = Vec::new();

    // 1. Conflict detection
    for (a, b, domain) in PM_PAIRS.iter().chain(LOG_PAIRS.iter()) {
        let a_nodes: Vec<String> = nodes.iter()
            .filter(|n| n.content.to_lowercase().contains(a))
            .map(|n| n.id.clone())
            .collect();
        let b_nodes: Vec<String> = nodes.iter()
            .filter(|n| n.content.to_lowercase().contains(b))
            .map(|n| n.id.clone())
            .collect();

        if !a_nodes.is_empty() && !b_nodes.is_empty() {
            let all_node_ids: Vec<String> = a_nodes.iter().chain(b_nodes.iter()).cloned().collect();
            errors.push(LintIssue {
                severity: LintSeverity::Error,
                rule: format!("conflict.{domain}"),
                message: format!("Conflicting {domain} rules: both '{}' and '{}' found", a, b),
                node_ids: all_node_ids,
            });
        }
    }

    // 2. Ambiguity linter
    for word in AMBIGUOUS_WORDS {
        let affected: Vec<String> = nodes.iter()
            .filter(|n| n.content.to_lowercase().contains(word))
            .map(|n| n.id.clone())
            .collect();

        if !affected.is_empty() {
            warnings.push(LintIssue {
                severity: LintSeverity::Warning,
                rule: "ambiguity.vague-language".to_string(),
                message: format!(
                    "Vague instruction word '{}' detected — use specific, testable language",
                    word
                ),
                node_ids: affected,
            });
        }
    }

    // 3. Budget check
    let total_bytes: usize = nodes.iter().map(|n| n.content.len()).sum();
    if total_bytes > budget_bytes {
        errors.push(LintIssue {
            severity: LintSeverity::Error,
            rule: "budget.exceeded".to_string(),
            message: format!(
                "Instruction content exceeds budget: {} bytes > {} bytes limit",
                total_bytes, budget_bytes
            ),
            node_ids: vec![],
        });
    } else if total_bytes > budget_bytes * 80 / 100 {
        warnings.push(LintIssue {
            severity: LintSeverity::Warning,
            rule: "budget.near-limit".to_string(),
            message: format!(
                "Instruction content at {}% of budget ({}/{} bytes)",
                total_bytes * 100 / budget_bytes, total_bytes, budget_bytes
            ),
            node_ids: vec![],
        });
    }

    let passed = errors.is_empty();
    LintReport { errors, warnings, passed }
}

#[derive(Debug)]
pub struct LintNode {
    pub id: String,
    pub content: String,
}
