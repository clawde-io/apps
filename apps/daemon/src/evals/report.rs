//! Regression report generator.
//!
//! Produces a Markdown document summarising eval results — pass/fail counts
//! and detailed diffs for every failed fixture.

use chrono::Utc;

use super::runner::EvalResult;

// ─── Report generation ────────────────────────────────────────────────────────

/// Generate a Markdown regression report from a slice of `EvalResult` records.
///
/// The report includes a summary table and an expanded section for every
/// fixture that did not pass.
pub fn generate_report(results: &[EvalResult]) -> String {
    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;
    let ts = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");

    let mut out = String::new();

    // Header
    out.push_str(&format!("# Eval Report — {}\n\n", ts));

    // Summary
    out.push_str("## Summary\n\n");
    out.push_str(&format!("- **Total**: {} fixtures\n", total));
    out.push_str(&format!("- **Passed**: {}\n", passed));
    out.push_str(&format!("- **Failed**: {}\n", failed));
    out.push('\n');

    if failed == 0 {
        out.push_str("All fixtures passed.\n");
        return out;
    }

    // Failures
    out.push_str("## Failures\n\n");
    for result in results.iter().filter(|r| !r.passed) {
        out.push_str(&format!("### {}\n\n", result.fixture));
        out.push_str(&format!(
            "- **Expected outcome**: `{}`\n",
            // We don't store expected on EvalResult; reconstruct from diffs.
            // The diff lines contain `expected X, got Y` already.
            "see diff"
        ));
        out.push_str(&format!(
            "- **Actual outcome**: `{}`\n",
            result.actual_outcome
        ));

        if !result.violations_found.is_empty() {
            out.push_str(&format!(
                "- **Violations found**: {}\n",
                result.violations_found.join(", ")
            ));
        }

        if !result.diffs.is_empty() {
            out.push_str("\n**Diffs:**\n\n");
            for diff in &result.diffs {
                out.push_str(&format!("- {}\n", diff));
            }
        }
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passing() -> EvalResult {
        EvalResult {
            fixture: "test_pass".to_string(),
            passed: true,
            actual_outcome: "allowed".to_string(),
            violations_found: Vec::new(),
            diffs: Vec::new(),
        }
    }

    fn failing() -> EvalResult {
        EvalResult {
            fixture: "test_fail".to_string(),
            passed: false,
            actual_outcome: "blocked".to_string(),
            violations_found: vec!["placeholder".to_string()],
            diffs: vec!["outcome: expected `allowed`, got `blocked`".to_string()],
        }
    }

    #[test]
    fn report_all_passed() {
        let report = generate_report(&[passing()]);
        assert!(report.contains("All fixtures passed."));
        assert!(report.contains("Passed**: 1"));
        assert!(report.contains("Failed**: 0"));
    }

    #[test]
    fn report_shows_failures() {
        let report = generate_report(&[passing(), failing()]);
        assert!(report.contains("test_fail"));
        assert!(report.contains("blocked"));
        assert!(report.contains("placeholder"));
        assert!(report.contains("Failures"));
    }
}
