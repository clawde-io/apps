// SPDX-License-Identifier: MIT
//! File context builder — truncates file content to ±N lines of relevance.
//!
//! When an AI session includes file content (e.g. via `@file` references or
//! repo context injection), the raw file can be hundreds of kilobytes.  This
//! module trims it to the most relevant region — a window of lines centered
//! on a focal line — keeping cost low without losing signal.
//!
//! # Defaults
//!
//! | Parameter | Default |
//! |-----------|---------|
//! | Context lines above focal | 100 |
//! | Context lines below focal | 100 |
//! | Max lines if no focal given | 200 |

/// Configuration for file context truncation.
#[derive(Debug, Clone)]
pub struct FileContextConfig {
    /// Lines to include above the focal line (inclusive).
    pub lines_before: usize,
    /// Lines to include below the focal line (inclusive).
    pub lines_after: usize,
    /// Maximum total lines to return when no focal line is given.
    pub max_lines_no_focal: usize,
}

impl Default for FileContextConfig {
    fn default() -> Self {
        Self {
            lines_before: 100,
            lines_after: 100,
            max_lines_no_focal: 200,
        }
    }
}

/// Truncate `content` to a window around `focal_line` (0-indexed).
///
/// If `focal_line` is `None`, the first `config.max_lines_no_focal` lines are
/// returned.
///
/// The returned string preserves the original line endings and never ends with
/// an extra newline.  A comment header is prepended when lines are omitted so
/// the AI knows the file was truncated:
///
/// ```text
/// // [file truncated — showing lines 45-145 of 800]
/// ```
///
/// # Arguments
///
/// * `content` — Full file content.
/// * `focal_line` — 0-indexed line number to center the window on.
/// * `config` — Window sizes.
pub fn truncate_file_context(
    content: &str,
    focal_line: Option<usize>,
    config: &FileContextConfig,
) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    if total == 0 {
        return String::new();
    }

    let (start, end) = match focal_line {
        Some(focal) => {
            let focal = focal.min(total.saturating_sub(1));
            let start = focal.saturating_sub(config.lines_before);
            let end = (focal + config.lines_after + 1).min(total);
            (start, end)
        }
        None => {
            let end = config.max_lines_no_focal.min(total);
            (0, end)
        }
    };

    let selected = &lines[start..end];
    let shown_start = start + 1; // 1-indexed for display
    let shown_end = end; // last line number shown

    let mut out = String::new();

    // Prepend truncation header when the file was actually cut.
    if start > 0 || end < total {
        out.push_str(&format!(
            "// [file truncated — showing lines {shown_start}-{shown_end} of {total}]\n"
        ));
    }

    out.push_str(&selected.join("\n"));
    out
}

/// Extract the line number of the first occurrence of `pattern` in `content`.
///
/// Returns 0-indexed line number.  Returns `None` if not found.
pub fn find_focal_line(content: &str, pattern: &str) -> Option<usize> {
    content
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(pattern))
        .map(|(i, _)| i)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_content(n: usize) -> String {
        (1..=n)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_empty_content_returns_empty() {
        let result = truncate_file_context("", None, &FileContextConfig::default());
        assert!(result.is_empty());
    }

    #[test]
    fn test_short_file_no_truncation() {
        let content = make_content(50);
        let config = FileContextConfig {
            lines_before: 100,
            lines_after: 100,
            max_lines_no_focal: 200,
        };
        let result = truncate_file_context(&content, None, &config);
        // No truncation header — all 50 lines fit.
        assert!(
            !result.contains("[file truncated"),
            "short file should not show header"
        );
        assert_eq!(result.lines().count(), 50);
    }

    #[test]
    fn test_no_focal_limits_to_max_lines() {
        let content = make_content(500);
        let config = FileContextConfig {
            lines_before: 100,
            lines_after: 100,
            max_lines_no_focal: 100,
        };
        let result = truncate_file_context(&content, None, &config);
        // Header line + 100 content lines = 101 lines.
        let line_count = result.lines().count();
        assert!(line_count <= 101, "got {line_count} lines");
        assert!(
            result.contains("[file truncated"),
            "should have truncation header"
        );
    }

    #[test]
    fn test_focal_line_centers_window() {
        let content = make_content(200);
        let config = FileContextConfig {
            lines_before: 5,
            lines_after: 5,
            max_lines_no_focal: 200,
        };
        // Focal = line 100 (0-indexed) → should include lines 95–105 (1-indexed).
        let result = truncate_file_context(&content, Some(99), &config);
        assert!(result.contains("line 95"), "should contain line 95");
        assert!(result.contains("line 105"), "should contain line 105");
        assert!(!result.contains("line 94"), "should NOT contain line 94");
        assert!(!result.contains("line 106"), "should NOT contain line 106");
    }

    #[test]
    fn test_focal_near_start_clips_safely() {
        let content = make_content(100);
        let config = FileContextConfig {
            lines_before: 50,
            lines_after: 10,
            max_lines_no_focal: 200,
        };
        // Focal at line 0 — no "before" lines exist.
        let result = truncate_file_context(&content, Some(0), &config);
        assert!(result.contains("line 1"), "should start at line 1");
    }

    #[test]
    fn test_focal_near_end_clips_safely() {
        let content = make_content(100);
        let config = FileContextConfig {
            lines_before: 10,
            lines_after: 50,
            max_lines_no_focal: 200,
        };
        // Focal at last line — no "after" lines exist.
        let result = truncate_file_context(&content, Some(99), &config);
        assert!(result.contains("line 100"), "should include last line");
    }

    #[test]
    fn test_truncation_header_format() {
        let content = make_content(1000);
        let config = FileContextConfig {
            lines_before: 10,
            lines_after: 10,
            max_lines_no_focal: 200,
        };
        let result = truncate_file_context(&content, Some(500), &config);
        assert!(
            result.starts_with("// [file truncated"),
            "header should be first line"
        );
        assert!(
            result.contains("of 1000"),
            "header should state total line count"
        );
    }

    #[test]
    fn test_find_focal_line_found() {
        let content = "fn main() {\n    let x = 1;\n    println!(\"{x}\");\n}";
        let line = find_focal_line(content, "println");
        assert_eq!(line, Some(2), "println is on line index 2");
    }

    #[test]
    fn test_find_focal_line_not_found() {
        let content = "no match here";
        let line = find_focal_line(content, "missing_symbol");
        assert_eq!(line, None);
    }

    // ── Security note (MI.T27) ────────────────────────────────────────────────
    // `truncate_file_context` and `find_focal_line` are pure string functions —
    // they never open files or resolve paths.  Path-traversal guards live in the
    // IPC handler layer before these functions are called.  These tests verify
    // the functions behave safely even if path-like content appears in a string.

    #[test]
    fn test_traversal_pattern_in_content_does_not_panic() {
        let content = "../../etc/passwd\nroot:x:0:0:root:/root:/bin/bash";
        let result = truncate_file_context(content, None, &FileContextConfig::default());
        assert!(!result.is_empty(), "should return content without panic");
    }

    #[test]
    fn test_find_focal_line_traversal_pattern() {
        let content = "fn safe() {}\n../../etc/shadow\nfn also_safe() {}";
        let line = find_focal_line(content, "../../etc/shadow");
        assert_eq!(line, Some(1), "pattern found on line index 1");
    }
}
