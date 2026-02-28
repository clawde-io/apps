//! Rule size budget — guard against bloated policy directories.
//!
//! Enforces soft limits:
//!   - Max 200 lines across all policy files
//!   - Max 20 policy files
//!
//! Violations are reported as warnings, not hard errors.

use std::path::Path;

use anyhow::{Context, Result};

/// Budget thresholds.
const MAX_LINES: u32 = 200;
const MAX_FILES: u32 = 20;

// ─── BudgetReport ─────────────────────────────────────────────────────────────

/// Summary of a policy directory budget check.
#[derive(Debug)]
pub struct BudgetReport {
    /// Total line count across all policy files.
    pub total_lines: u32,
    /// Total number of policy files.
    pub total_files: u32,
    /// True if `total_lines > MAX_LINES`.
    pub exceeds_line_budget: bool,
    /// True if `total_files > MAX_FILES`.
    pub exceeds_file_budget: bool,
    /// Human-readable warnings for each limit exceeded.
    pub warnings: Vec<String>,
}

// ─── Checking ─────────────────────────────────────────────────────────────────

/// Check the policy directory against the size budget.
///
/// Returns a `BudgetReport` regardless of whether limits are exceeded.
/// Callers should log `warnings` when `exceeds_*` flags are set.
pub async fn check_rule_budget(policies_dir: &Path) -> Result<BudgetReport> {
    if !policies_dir.exists() {
        return Ok(BudgetReport {
            total_lines: 0,
            total_files: 0,
            exceeds_line_budget: false,
            exceeds_file_budget: false,
            warnings: Vec::new(),
        });
    }

    let mut entries = tokio::fs::read_dir(policies_dir)
        .await
        .with_context(|| format!("read policies dir: {}", policies_dir.display()))?;

    let mut total_lines: u32 = 0;
    let mut total_files: u32 = 0;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        total_files += 1;
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("read policy file: {}", path.display()))?;
        total_lines += content.lines().count() as u32;
    }

    let exceeds_line_budget = total_lines > MAX_LINES;
    let exceeds_file_budget = total_files > MAX_FILES;

    let mut warnings = Vec::new();
    if exceeds_line_budget {
        warnings.push(format!(
            "Policy line budget exceeded: {} lines (max {})",
            total_lines, MAX_LINES
        ));
    }
    if exceeds_file_budget {
        warnings.push(format!(
            "Policy file count exceeded: {} files (max {})",
            total_files, MAX_FILES
        ));
    }

    Ok(BudgetReport {
        total_lines,
        total_files,
        exceeds_line_budget,
        exceeds_file_budget,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn nonexistent_dir_returns_zero() {
        let report = check_rule_budget(Path::new("/nonexistent/path/policies"))
            .await
            .unwrap();
        assert_eq!(report.total_files, 0);
        assert_eq!(report.total_lines, 0);
        assert!(!report.exceeds_file_budget);
        assert!(!report.exceeds_line_budget);
    }
}
