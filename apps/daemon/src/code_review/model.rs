// SPDX-License-Identifier: MIT
//! Data models for the AI Code Review Engine.
//!
//! All types are `Serialize`/`Deserialize` so they can be sent over JSON-RPC
//! and stored in the `review_results` and `review_feedback` SQLite tables.

use serde::{Deserialize, Serialize};

// ─── Configuration ────────────────────────────────────────────────────────────

/// Per-repo review configuration, typically loaded from `.clawde.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewConfig {
    /// Ordered list of lint/analysis tools to run.
    pub tools: Vec<ToolConfig>,
    /// Minimum severity that is included in the review output.
    pub severity_threshold: ReviewSeverity,
    /// Triggers that automatically start a review: `"pr"`, `"commit"`, `"manual"`.
    pub auto_run_on: Vec<String>,
    /// File/directory globs that are excluded from review.
    pub ignore_paths: Vec<String>,
    /// Minimum grade required to pass (used by CI integration).
    pub require_grade: Option<Grade>,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            tools: vec![
                ToolConfig::clippy(),
                ToolConfig::eslint(),
                ToolConfig::flutter_analyze(),
            ],
            severity_threshold: ReviewSeverity::Warning,
            auto_run_on: vec!["manual".to_string()],
            ignore_paths: vec![
                "target/".to_string(),
                "node_modules/".to_string(),
                ".git/".to_string(),
            ],
            require_grade: None,
        }
    }
}

/// Configuration for a single external lint/analysis tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Human-readable tool name (e.g. `"clippy"`, `"eslint"`).
    pub name: String,
    /// Command and arguments to execute. The first element is the binary.
    /// Use `{repo_path}` as a placeholder for the repository root.
    pub command: Vec<String>,
    /// Expected output format: `"eslint-json"`, `"clippy-json"`, `"flutter-text"`, `"golangci-json"`, `"pylint-json"`, `"semgrep-json"`.
    pub output_format: String,
    /// Whether this tool is currently enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl ToolConfig {
    /// Default Clippy configuration for Rust projects.
    pub fn clippy() -> Self {
        Self {
            name: "clippy".to_string(),
            command: vec![
                "cargo".to_string(),
                "clippy".to_string(),
                "--message-format=json".to_string(),
                "--".to_string(),
                "-D".to_string(),
                "warnings".to_string(),
            ],
            output_format: "clippy-json".to_string(),
            enabled: true,
        }
    }

    /// Default ESLint configuration for JavaScript/TypeScript projects.
    pub fn eslint() -> Self {
        Self {
            name: "eslint".to_string(),
            command: vec![
                "npx".to_string(),
                "eslint".to_string(),
                "--format=json".to_string(),
                ".".to_string(),
            ],
            output_format: "eslint-json".to_string(),
            enabled: true,
        }
    }

    /// Default `flutter analyze` configuration for Dart projects.
    pub fn flutter_analyze() -> Self {
        Self {
            name: "flutter_analyze".to_string(),
            command: vec!["flutter".to_string(), "analyze".to_string()],
            output_format: "flutter-text".to_string(),
            enabled: true,
        }
    }

    /// Default golangci-lint configuration for Go projects.
    pub fn golangci() -> Self {
        Self {
            name: "golangci-lint".to_string(),
            command: vec![
                "golangci-lint".to_string(),
                "run".to_string(),
                "--out-format=json".to_string(),
            ],
            output_format: "golangci-json".to_string(),
            enabled: false,
        }
    }
}

// ─── Review issue (raw tool finding) ─────────────────────────────────────────

/// A single finding from a lint/analysis tool, before AI synthesis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewIssue {
    /// Repository-relative file path.
    pub file: String,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number (optional — not all tools report columns).
    pub col: Option<u32>,
    /// Issue severity.
    pub severity: ReviewSeverity,
    /// Tool that emitted this issue (e.g. `"clippy"`, `"eslint"`).
    pub tool: String,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Optional machine-applicable fix suggestion (e.g. Clippy `--fix`).
    pub fix_suggestion: Option<String>,
    /// Diagnostic code or rule ID (e.g. `"E0001"`, `"no-unused-vars"`).
    pub code: Option<String>,
}

// ─── AI-synthesized review comment ───────────────────────────────────────────

/// A high-level review comment produced by AI synthesis from raw tool findings.
///
/// Groups multiple `ReviewIssue`s by theme and adds developer-facing context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewComment {
    /// Repository-relative file path this comment is anchored to, if any.
    pub file: Option<String>,
    /// Thematic grouping: `"security"`, `"performance"`, `"correctness"`, `"style"`, `"breaking"`.
    pub theme: String,
    /// Aggregate severity of this comment.
    pub severity: ReviewSeverity,
    /// AI-generated explanation in plain English.
    pub explanation: String,
    /// Ordered list of concrete suggestions.
    pub suggestions: Vec<String>,
    /// Whether the AI flagged this as potentially a false positive.
    pub is_uncertain: bool,
}

// ─── Tool run result ──────────────────────────────────────────────────────────

/// The raw result of running a single tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Tool name (matches `ToolConfig::name`).
    pub tool: String,
    /// Whether the tool ran successfully (exit code 0 or 1 for lint warnings are both OK).
    pub success: bool,
    /// Raw stdout from the tool (truncated to 64 KiB to avoid OOM).
    pub raw_output: String,
    /// Number of issues parsed from the output.
    pub issue_count: usize,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Error message if the tool failed to run.
    pub error: Option<String>,
}

// ─── Review result ────────────────────────────────────────────────────────────

/// Complete result of a review run, returned by `review.run`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    /// Unique review ID (UUID v4).
    pub id: String,
    /// Letter grade computed from the findings.
    pub grade: Grade,
    /// AI-generated executive summary of the review.
    pub summary: String,
    /// AI-synthesized comments grouped by theme.
    pub comments: Vec<ReviewComment>,
    /// Raw results from each tool (for debugging/transparency).
    pub tool_results: Vec<ToolResult>,
    /// ISO-8601 timestamp when the review was created.
    pub created_at: String,
    /// Total number of raw issues found across all tools.
    pub total_issues: usize,
    /// Number of error-severity issues.
    pub error_count: usize,
    /// Number of warning-severity issues.
    pub warning_count: usize,
}

// ─── Grade ────────────────────────────────────────────────────────────────────

/// Letter grade assigned to a code review based on findings.
///
/// Grading rubric:
/// - `A` — zero errors and zero warnings
/// - `B` — zero errors, up to 5 warnings
/// - `C` — zero errors, up to 15 warnings
/// - `D` — 1–3 errors (or >15 warnings)
/// - `F` — 4+ errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Grade {
    A,
    B,
    C,
    D,
    F,
}

impl Grade {
    /// Compute a grade from raw error/warning counts.
    pub fn from_counts(errors: usize, warnings: usize) -> Self {
        if errors >= 4 {
            Grade::F
        } else if errors >= 1 {
            Grade::D
        } else if warnings == 0 {
            Grade::A
        } else if warnings <= 5 {
            Grade::B
        } else if warnings <= 15 {
            Grade::C
        } else {
            Grade::D
        }
    }

    /// Returns `true` if this grade is at least as good as `required`.
    pub fn meets(self, required: Grade) -> bool {
        // Ordinal: A=0, B=1, C=2, D=3, F=4 — lower is better.
        fn ord(g: Grade) -> u8 {
            match g {
                Grade::A => 0,
                Grade::B => 1,
                Grade::C => 2,
                Grade::D => 3,
                Grade::F => 4,
            }
        }
        ord(self) <= ord(required)
    }
}

impl std::fmt::Display for Grade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Grade::A => write!(f, "A"),
            Grade::B => write!(f, "B"),
            Grade::C => write!(f, "C"),
            Grade::D => write!(f, "D"),
            Grade::F => write!(f, "F"),
        }
    }
}

// ─── Severity ─────────────────────────────────────────────────────────────────

/// Issue severity level, aligned across all tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewSeverity {
    /// Informational — cosmetic or style suggestions.
    Hint,
    /// Informational — worth knowing.
    Info,
    /// Potential problem — should be fixed.
    Warning,
    /// Definite problem — must be fixed.
    Error,
}

impl ReviewSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReviewSeverity::Hint => "hint",
            ReviewSeverity::Info => "info",
            ReviewSeverity::Warning => "warning",
            ReviewSeverity::Error => "error",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "error" | "err" | "fatal" => ReviewSeverity::Error,
            "warning" | "warn" => ReviewSeverity::Warning,
            "info" | "note" | "information" => ReviewSeverity::Info,
            _ => ReviewSeverity::Hint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grade_from_counts() {
        assert_eq!(Grade::from_counts(0, 0), Grade::A);
        assert_eq!(Grade::from_counts(0, 3), Grade::B);
        assert_eq!(Grade::from_counts(0, 6), Grade::C);
        assert_eq!(Grade::from_counts(1, 0), Grade::D);
        assert_eq!(Grade::from_counts(4, 0), Grade::F);
        assert_eq!(Grade::from_counts(10, 100), Grade::F);
    }

    #[test]
    fn test_grade_meets() {
        assert!(Grade::A.meets(Grade::B));
        assert!(Grade::B.meets(Grade::B));
        assert!(!Grade::C.meets(Grade::B));
        assert!(Grade::F.meets(Grade::F));
        assert!(!Grade::F.meets(Grade::A));
    }

    #[test]
    fn test_severity_ordering() {
        assert!(ReviewSeverity::Error > ReviewSeverity::Warning);
        assert!(ReviewSeverity::Warning > ReviewSeverity::Info);
        assert!(ReviewSeverity::Info > ReviewSeverity::Hint);
    }
}
