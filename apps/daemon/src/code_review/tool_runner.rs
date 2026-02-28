// SPDX-License-Identifier: MIT
//! Tool runner — spawn external lint/analysis tools, parse output, aggregate findings.
//!
//! Supported output formats (v0.2.0):
//! - `clippy-json`    — cargo clippy --message-format=json
//! - `eslint-json`    — eslint --format=json
//! - `flutter-text`   — flutter analyze (text output)
//! - `golangci-json`  — golangci-lint --out-format=json
//! - `pylint-json`    — pylint --output-format=json
//! - `semgrep-json`   — semgrep --json

use crate::code_review::model::{ReviewIssue, ReviewSeverity, ToolConfig, ToolResult};
use anyhow::Result;
use std::path::Path;
use std::time::Instant;
use tokio::process::Command;
use tracing::{debug, warn};

/// Maximum captured stdout size (64 KiB). Prevents OOM from runaway tool output.
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Per-tool execution timeout.
const TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

// ─── ToolRunner ───────────────────────────────────────────────────────────────

/// Runs external lint tools as subprocesses and parses their output.
pub struct ToolRunner;

impl ToolRunner {
    /// Run a single tool against `repo_path`.
    ///
    /// `diff` is an optional unified diff string. When provided, some parsers
    /// can use it to restrict findings to changed lines only (future enhancement).
    pub async fn run_tool(
        config: &ToolConfig,
        repo_path: &Path,
        _diff: Option<&str>,
    ) -> Result<(Vec<ReviewIssue>, ToolResult)> {
        if !config.enabled {
            let result = ToolResult {
                tool: config.name.clone(),
                success: true,
                raw_output: String::new(),
                issue_count: 0,
                duration_ms: 0,
                error: None,
            };
            return Ok((vec![], result));
        }

        debug!(tool = %config.name, "running tool");
        let start = Instant::now();

        // Build command — first element is binary, rest are args.
        let mut cmd_parts = config.command.iter();
        let binary = match cmd_parts.next() {
            Some(b) => b,
            None => {
                let result = ToolResult {
                    tool: config.name.clone(),
                    success: false,
                    raw_output: String::new(),
                    issue_count: 0,
                    duration_ms: 0,
                    error: Some("tool command is empty".to_string()),
                };
                return Ok((vec![], result));
            }
        };

        let args: Vec<&str> = cmd_parts.map(|s| s.as_str()).collect();

        let run = tokio::time::timeout(TOOL_TIMEOUT, async {
            Command::new(binary)
                .args(&args)
                .current_dir(repo_path)
                .output()
                .await
        })
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        let output = match run {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => {
                warn!(tool = %config.name, err = %e, "tool spawn failed");
                let result = ToolResult {
                    tool: config.name.clone(),
                    success: false,
                    raw_output: String::new(),
                    issue_count: 0,
                    duration_ms,
                    error: Some(format!("spawn error: {}", e)),
                };
                return Ok((vec![], result));
            }
            Err(_) => {
                warn!(tool = %config.name, "tool timed out after 5 minutes");
                let result = ToolResult {
                    tool: config.name.clone(),
                    success: false,
                    raw_output: String::new(),
                    issue_count: 0,
                    duration_ms,
                    error: Some("timed out after 300 seconds".to_string()),
                };
                return Ok((vec![], result));
            }
        };

        // Capture stdout, truncate to MAX_OUTPUT_BYTES.
        let raw = {
            let bytes = &output.stdout;
            if bytes.len() > MAX_OUTPUT_BYTES {
                warn!(tool = %config.name, bytes = bytes.len(), "truncating large output");
                String::from_utf8_lossy(&bytes[..MAX_OUTPUT_BYTES]).into_owned()
            } else {
                String::from_utf8_lossy(bytes).into_owned()
            }
        };

        // Parse issues — gracefully handle malformed output.
        let issues = parse_output(&config.name, &config.output_format, &raw, repo_path);
        let issue_count = issues.len();

        // Exit codes: 0 = no issues, 1 = issues found (both are "success" for our purposes).
        // Only treat the tool as failed if it couldn't run (exit code > 1, or spawn error).
        let success = output.status.code().map(|c| c <= 1).unwrap_or(false);

        if !success {
            let stderr_preview =
                String::from_utf8_lossy(&output.stderr[..output.stderr.len().min(512)]);
            warn!(tool = %config.name, code = ?output.status.code(), stderr = %stderr_preview, "tool exited with error");
        }

        let result = ToolResult {
            tool: config.name.clone(),
            success,
            raw_output: raw,
            issue_count,
            duration_ms,
            error: if !success {
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                Some(stderr[..stderr.len().min(512)].to_string())
            } else {
                None
            },
        };

        Ok((issues, result))
    }

    /// Run all enabled tools in `configs` and return the aggregate issues list
    /// and per-tool results.
    pub async fn run_all(
        configs: &[ToolConfig],
        repo_path: &Path,
        diff: Option<&str>,
    ) -> (Vec<ReviewIssue>, Vec<ToolResult>) {
        let mut all_issues: Vec<ReviewIssue> = Vec::new();
        let mut all_results: Vec<ToolResult> = Vec::new();

        for config in configs {
            match Self::run_tool(config, repo_path, diff).await {
                Ok((issues, result)) => {
                    all_issues.extend(issues);
                    all_results.push(result);
                }
                Err(e) => {
                    warn!(tool = %config.name, err = %e, "tool runner error");
                    all_results.push(ToolResult {
                        tool: config.name.clone(),
                        success: false,
                        raw_output: String::new(),
                        issue_count: 0,
                        duration_ms: 0,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        (all_issues, all_results)
    }
}

// ─── Output parsers ───────────────────────────────────────────────────────────

/// Dispatch to the correct parser based on `output_format`.
///
/// Never panics — returns an empty list if the output is malformed.
fn parse_output(tool: &str, format: &str, raw: &str, repo_path: &Path) -> Vec<ReviewIssue> {
    let result = match format {
        "clippy-json" => parse_clippy_json(raw, tool),
        "eslint-json" => parse_eslint_json(raw, tool),
        "flutter-text" => parse_flutter_text(raw, tool),
        "golangci-json" => parse_golangci_json(raw, tool),
        "pylint-json" => parse_pylint_json(raw, tool),
        "semgrep-json" => parse_semgrep_json(raw, tool),
        other => {
            warn!(
                tool,
                format = other,
                "unknown output format — skipping parse"
            );
            return vec![];
        }
    };

    match result {
        Ok(mut issues) => {
            // Make file paths repo-relative.
            for issue in &mut issues {
                if let Ok(rel) = Path::new(&issue.file).strip_prefix(repo_path) {
                    issue.file = rel.to_string_lossy().into_owned();
                }
            }
            issues
        }
        Err(e) => {
            warn!(tool, format, err = %e, "failed to parse tool output — treating as zero findings");
            vec![]
        }
    }
}

/// Parse `cargo clippy --message-format=json` output.
///
/// Each line is a JSON object. We only care about lines with `"reason": "compiler-message"`.
fn parse_clippy_json(raw: &str, tool: &str) -> Result<Vec<ReviewIssue>> {
    let mut issues = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let obj: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue, // skip malformed lines
        };

        if obj.get("reason").and_then(|v| v.as_str()) != Some("compiler-message") {
            continue;
        }

        let msg = match obj.get("message") {
            Some(m) => m,
            None => continue,
        };

        let level = msg
            .get("level")
            .and_then(|v| v.as_str())
            .unwrap_or("warning");
        let severity = ReviewSeverity::from_str(level);

        let message_text = msg
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown diagnostic")
            .to_string();

        let code = msg
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Extract primary span.
        let spans = msg.get("spans").and_then(|v| v.as_array());
        if let Some(spans) = spans {
            for span in spans {
                let is_primary = span
                    .get("is_primary")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !is_primary {
                    continue;
                }
                let file = span
                    .get("file_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let line = span.get("line_start").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                let col = span
                    .get("column_start")
                    .and_then(|v| v.as_u64())
                    .map(|c| c as u32);

                // Look for a fix suggestion in the span.
                let fix_suggestion =
                    msg.get("children")
                        .and_then(|v| v.as_array())
                        .and_then(|children| {
                            children.iter().find_map(|child| {
                                let child_msg = child.get("message")?.as_str()?;
                                if child_msg.starts_with("help:") || child_msg.starts_with("note:")
                                {
                                    Some(child_msg.to_string())
                                } else {
                                    None
                                }
                            })
                        });

                issues.push(ReviewIssue {
                    file,
                    line,
                    col,
                    severity,
                    tool: tool.to_string(),
                    message: message_text.clone(),
                    fix_suggestion,
                    code: code.clone(),
                });
                break; // only the primary span
            }
        } else {
            // No spans — emit a file-less issue.
            issues.push(ReviewIssue {
                file: String::new(),
                line: 1,
                col: None,
                severity,
                tool: tool.to_string(),
                message: message_text,
                fix_suggestion: None,
                code,
            });
        }
    }

    Ok(issues)
}

/// Parse `eslint --format=json` output.
///
/// Top-level is a JSON array of file results, each with a `messages` array.
fn parse_eslint_json(raw: &str, tool: &str) -> Result<Vec<ReviewIssue>> {
    let root: serde_json::Value = serde_json::from_str(raw.trim())?;
    let files = root
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("expected array"))?;
    let mut issues = Vec::new();

    for file_obj in files {
        let file_path = file_obj
            .get("filePath")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let messages = match file_obj.get("messages").and_then(|v| v.as_array()) {
            Some(m) => m,
            None => continue,
        };

        for msg in messages {
            let severity_num = msg.get("severity").and_then(|v| v.as_u64()).unwrap_or(1);
            let severity = match severity_num {
                2 => ReviewSeverity::Error,
                1 => ReviewSeverity::Warning,
                _ => ReviewSeverity::Info,
            };

            let message = msg
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let line = msg.get("line").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let col = msg.get("column").and_then(|v| v.as_u64()).map(|c| c as u32);
            let code = msg
                .get("ruleId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let fix_suggestion = msg
                .get("fix")
                .and_then(|f| f.get("text"))
                .and_then(|t| t.as_str())
                .map(|s| format!("Auto-fix available: {}", s));

            issues.push(ReviewIssue {
                file: file_path.clone(),
                line,
                col,
                severity,
                tool: tool.to_string(),
                message,
                fix_suggestion,
                code,
            });
        }
    }

    Ok(issues)
}

/// Parse `flutter analyze` text output.
///
/// Each issue line has the format:
///   `  • message • file_path:line:col • diagnostic_code`
///   `  error • message • file_path:line:col • code`
fn parse_flutter_text(raw: &str, tool: &str) -> Result<Vec<ReviewIssue>> {
    let mut issues = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        // Flutter analyze issues start with a severity indicator.
        if !line.starts_with("error")
            && !line.starts_with("warning")
            && !line.starts_with("info")
            && !line.starts_with("hint")
            && !line.starts_with('•')
        {
            continue;
        }

        // Parts are separated by " • "
        let parts: Vec<&str> = line.split(" • ").collect();
        if parts.len() < 3 {
            continue;
        }

        let (severity_str, message) = if parts.len() >= 4 {
            // "error • message • file:line:col • code"
            (parts[0].trim(), parts[1].trim().to_string())
        } else {
            // "• message • file:line:col"
            ("warning", parts[1].trim().to_string())
        };

        let severity = ReviewSeverity::from_str(severity_str);

        // Location part: "file_path:line:col"
        let loc_part = parts[parts
            .len()
            .saturating_sub(if parts.len() >= 4 { 2 } else { 1 })];
        let (file, line_num, col_num) = parse_file_location(loc_part);

        let code = if parts.len() >= 4 {
            Some(parts[parts.len() - 1].trim().to_string())
        } else {
            None
        };

        issues.push(ReviewIssue {
            file,
            line: line_num,
            col: col_num,
            severity,
            tool: tool.to_string(),
            message,
            fix_suggestion: None,
            code,
        });
    }

    Ok(issues)
}

/// Parse `golangci-lint --out-format=json` output.
fn parse_golangci_json(raw: &str, tool: &str) -> Result<Vec<ReviewIssue>> {
    let root: serde_json::Value = serde_json::from_str(raw.trim())?;
    let mut issues = Vec::new();

    let issue_list = root
        .get("Issues")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("missing Issues array"))?;

    for item in issue_list {
        let message = item
            .get("Text")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let linter = item
            .get("FromLinter")
            .and_then(|v| v.as_str())
            .unwrap_or(tool);

        let pos = item.get("Pos");
        let file = pos
            .and_then(|p| p.get("Filename"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let line = pos
            .and_then(|p| p.get("Line"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;
        let col = pos
            .and_then(|p| p.get("Column"))
            .and_then(|v| v.as_u64())
            .map(|c| c as u32);

        // golangci-lint maps most issues to warning severity.
        let severity = ReviewSeverity::Warning;

        issues.push(ReviewIssue {
            file,
            line,
            col,
            severity,
            tool: format!("{}({})", tool, linter),
            message,
            fix_suggestion: None,
            code: Some(linter.to_string()),
        });
    }

    Ok(issues)
}

/// Parse `pylint --output-format=json` output.
fn parse_pylint_json(raw: &str, tool: &str) -> Result<Vec<ReviewIssue>> {
    let root: serde_json::Value = serde_json::from_str(raw.trim())?;
    let items = root
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("expected array"))?;
    let mut issues = Vec::new();

    for item in items {
        let message = item
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let file = item
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let line = item.get("line").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
        let col = item
            .get("column")
            .and_then(|v| v.as_u64())
            .map(|c| c as u32);
        let msg_type = item
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("warning");
        let code = item
            .get("message-id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let severity = match msg_type {
            "error" | "fatal" => ReviewSeverity::Error,
            "warning" => ReviewSeverity::Warning,
            "refactor" | "convention" => ReviewSeverity::Info,
            _ => ReviewSeverity::Hint,
        };

        issues.push(ReviewIssue {
            file,
            line,
            col,
            severity,
            tool: tool.to_string(),
            message,
            fix_suggestion: None,
            code,
        });
    }

    Ok(issues)
}

/// Parse `semgrep --json` output.
fn parse_semgrep_json(raw: &str, tool: &str) -> Result<Vec<ReviewIssue>> {
    let root: serde_json::Value = serde_json::from_str(raw.trim())?;
    let results = root
        .get("results")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("missing results array"))?;
    let mut issues = Vec::new();

    for item in results {
        let message = item
            .get("extra")
            .and_then(|e| e.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let file = item
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let start = item.get("start");
        let line = start
            .and_then(|s| s.get("line"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;
        let col = start
            .and_then(|s| s.get("col"))
            .and_then(|v| v.as_u64())
            .map(|c| c as u32);

        let severity_str = item
            .get("extra")
            .and_then(|e| e.get("severity"))
            .and_then(|v| v.as_str())
            .unwrap_or("warning");
        let severity = ReviewSeverity::from_str(severity_str);

        let code = item
            .get("check_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        issues.push(ReviewIssue {
            file,
            line,
            col,
            severity,
            tool: tool.to_string(),
            message,
            fix_suggestion: None,
            code,
        });
    }

    Ok(issues)
}

// ─── Aggregation ──────────────────────────────────────────────────────────────

/// Aggregate issues from multiple tools, deduplicating overlapping findings.
///
/// Two issues are considered duplicates if they share the same file, line,
/// and a message prefix of 40 characters. When duplicates are found, the
/// higher-severity issue is kept and the tool names are merged.
pub fn aggregate_results(results: Vec<Vec<ReviewIssue>>) -> Vec<ReviewIssue> {
    let mut flat: Vec<ReviewIssue> = results.into_iter().flatten().collect();
    let mut seen: Vec<ReviewIssue> = Vec::new();

    for issue in flat.drain(..) {
        let msg_prefix: String = issue.message.chars().take(40).collect();

        if let Some(existing) = seen.iter_mut().find(|s| {
            s.file == issue.file && s.line == issue.line && s.message.starts_with(&msg_prefix)
        }) {
            // Merge: keep the higher severity, combine tool names.
            if issue.severity > existing.severity {
                existing.severity = issue.severity;
            }
            if !existing.tool.contains(&issue.tool) {
                existing.tool = format!("{},{}", existing.tool, issue.tool);
            }
        } else {
            seen.push(issue);
        }
    }

    // Sort: errors first, then by file + line.
    seen.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
    });

    seen
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Parse a location string of the form `"file/path.ext:line:col"`.
/// Returns `(file, line, col)`. Missing parts default to `("", 1, None)`.
fn parse_file_location(loc: &str) -> (String, u32, Option<u32>) {
    let parts: Vec<&str> = loc.rsplitn(3, ':').collect();
    // rsplitn gives us [col, line, file] (reversed).
    match parts.len() {
        3 => {
            let file = parts[2].to_string();
            let line = parts[1].parse::<u32>().unwrap_or(1);
            let col = parts[0].parse::<u32>().ok();
            (file, line, col)
        }
        2 => {
            let file = parts[1].to_string();
            let line = parts[0].parse::<u32>().unwrap_or(1);
            (file, line, None)
        }
        _ => (loc.to_string(), 1, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_eslint_json_well_formed() {
        let raw = r#"[
            {
                "filePath": "/project/src/index.ts",
                "messages": [
                    {
                        "ruleId": "no-unused-vars",
                        "severity": 1,
                        "message": "'x' is defined but never used.",
                        "line": 10,
                        "column": 5
                    },
                    {
                        "ruleId": "no-console",
                        "severity": 2,
                        "message": "Unexpected console statement.",
                        "line": 20,
                        "column": 1
                    }
                ]
            }
        ]"#;

        let issues = parse_eslint_json(raw, "eslint").expect("parse should succeed");
        assert_eq!(issues.len(), 2);

        let warning = &issues[0];
        assert_eq!(warning.severity, ReviewSeverity::Warning);
        assert_eq!(warning.line, 10);
        assert_eq!(warning.col, Some(5));
        assert_eq!(warning.code.as_deref(), Some("no-unused-vars"));

        let error = &issues[1];
        assert_eq!(error.severity, ReviewSeverity::Error);
        assert_eq!(error.line, 20);
    }

    #[test]
    fn test_parse_eslint_json_empty_array() {
        let raw = r#"[]"#;
        let issues = parse_eslint_json(raw, "eslint").expect("parse should succeed");
        assert!(issues.is_empty());
    }

    #[test]
    fn test_parse_eslint_json_malformed_returns_empty() {
        let raw = "this is not json {{{";
        let issues = parse_output("eslint", "eslint-json", raw, Path::new("/project"));
        assert!(
            issues.is_empty(),
            "malformed output should produce no issues"
        );
    }

    #[test]
    fn test_aggregate_deduplicates_overlapping_findings() {
        // Both messages share the same first 40 chars — the dedup key used by
        // aggregate_results (existing.message.starts_with(current_issue_prefix)).
        let shared_msg_a = "unused variable `x` is never read; consider removing it [clippy]";
        let shared_msg_b = "unused variable `x` is never read; consider removing it [semgrep]";
        let issue_a = ReviewIssue {
            file: "src/main.rs".to_string(),
            line: 42,
            col: Some(5),
            severity: ReviewSeverity::Warning,
            tool: "clippy".to_string(),
            message: shared_msg_a.to_string(),
            fix_suggestion: None,
            code: Some("unused_variables".to_string()),
        };
        let issue_b = ReviewIssue {
            file: "src/main.rs".to_string(),
            line: 42,
            col: Some(5),
            severity: ReviewSeverity::Error,
            tool: "semgrep".to_string(),
            message: shared_msg_b.to_string(),
            fix_suggestion: None,
            code: Some("rust.unused-var".to_string()),
        };

        let aggregated = aggregate_results(vec![vec![issue_a], vec![issue_b]]);
        assert_eq!(aggregated.len(), 1, "duplicate finding should be merged");
        assert_eq!(
            aggregated[0].severity,
            ReviewSeverity::Error,
            "higher severity wins"
        );
        assert!(
            aggregated[0].tool.contains("clippy"),
            "merged tool list should include clippy"
        );
        assert!(
            aggregated[0].tool.contains("semgrep"),
            "merged tool list should include semgrep"
        );
    }

    #[test]
    fn test_aggregate_keeps_distinct_findings() {
        let issue_a = ReviewIssue {
            file: "src/a.rs".to_string(),
            line: 10,
            col: None,
            severity: ReviewSeverity::Warning,
            tool: "clippy".to_string(),
            message: "first warning".to_string(),
            fix_suggestion: None,
            code: None,
        };
        let issue_b = ReviewIssue {
            file: "src/b.rs".to_string(),
            line: 20,
            col: None,
            severity: ReviewSeverity::Error,
            tool: "eslint".to_string(),
            message: "second finding completely different".to_string(),
            fix_suggestion: None,
            code: None,
        };

        let aggregated = aggregate_results(vec![vec![issue_a], vec![issue_b]]);
        assert_eq!(aggregated.len(), 2, "distinct findings should both be kept");
    }

    #[test]
    fn test_parse_file_location() {
        assert_eq!(
            parse_file_location("src/main.rs:42:7"),
            ("src/main.rs".to_string(), 42, Some(7))
        );
        assert_eq!(
            parse_file_location("src/main.rs:42"),
            ("src/main.rs".to_string(), 42, None)
        );
        assert_eq!(
            parse_file_location("src/main.rs"),
            ("src/main.rs".to_string(), 1, None)
        );
    }
}
