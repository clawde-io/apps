//! "No placeholders" scanner.
//!
//! Detects unimplemented stubs left in code — TODO, FIXME, `unimplemented!()`,
//! `todo!()`, Python's `pass`, `raise NotImplementedError`, etc.
//!
//! Can scan a unified diff (added lines only) or file contents directly.

// ─── Pattern table ────────────────────────────────────────────────────────────

const PLACEHOLDER_PATTERNS: &[&str] = &[
    "TODO",
    "FIXME",
    "STUB",
    "placeholder",
    "implement_here",
    "pass\n",
    "pass\r\n",
    "unimplemented!()",
    "todo!()",
    "NotImplementedError",
    "throw new Error('not implemented')",
    "throw new Error(\"not implemented\")",
    "raise NotImplementedError",
];

// ─── Violation type ───────────────────────────────────────────────────────────

/// A single placeholder violation found in a file or patch.
#[derive(Debug, Clone)]
pub struct PlaceholderViolation {
    /// File path where the violation was found (may be empty for patch scans).
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    /// The placeholder pattern that matched.
    pub pattern: String,
    /// The full source line containing the violation.
    pub context: String,
}

// ─── Patch scanning ───────────────────────────────────────────────────────────

/// Scan a unified diff for placeholder patterns.
///
/// Only added lines (lines starting with `+` but not `+++`) are inspected.
/// Returns one `PlaceholderViolation` per matching line.
pub fn scan_patch(patch: &str) -> Vec<PlaceholderViolation> {
    let mut violations = Vec::new();
    let mut current_file = String::new();
    let mut line_num: usize = 0;

    for line in patch.lines() {
        // Track current file from diff header.
        if line.starts_with("+++ b/") || line.starts_with("+++ ") {
            current_file = line
                .trim_start_matches("+++ b/")
                .trim_start_matches("+++ ")
                .to_string();
            line_num = 0;
            continue;
        }
        if line.starts_with("@@") {
            // Parse the destination line start from `@@ -a,b +c,d @@`
            if let Some(plus_part) = line.split('+').nth(1) {
                line_num = plus_part
                    .split([',', ' '])
                    .next()
                    .and_then(|n| n.parse::<usize>().ok())
                    .unwrap_or(0);
            }
            continue;
        }
        if line.starts_with('+') && !line.starts_with("+++") {
            line_num += 1;
            let content = &line[1..]; // strip the leading `+`
            for &pattern in PLACEHOLDER_PATTERNS {
                if content.contains(pattern) {
                    violations.push(PlaceholderViolation {
                        file: current_file.clone(),
                        line: line_num,
                        pattern: pattern.to_string(),
                        context: content.to_string(),
                    });
                    break; // one violation per line is enough
                }
            }
        } else if line.starts_with(' ') {
            // Context line — still advances the destination line counter.
            line_num += 1;
        }
        // Lines starting with `-` are removed lines; they don't advance dest counter.
    }

    violations
}

// ─── Content scanning ────────────────────────────────────────────────────────

/// Scan the full content of a file for placeholder patterns.
///
/// Returns one `PlaceholderViolation` per matching line.
pub fn scan_content(content: &str, file_name: &str) -> Vec<PlaceholderViolation> {
    let mut violations = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        for &pattern in PLACEHOLDER_PATTERNS {
            if line.contains(pattern) {
                violations.push(PlaceholderViolation {
                    file: file_name.to_string(),
                    line: idx + 1,
                    pattern: pattern.to_string(),
                    context: line.to_string(),
                });
                break;
            }
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_content_finds_todo() {
        let content = "fn foo() {\n    // TODO: implement this\n}\n";
        let violations = scan_content(content, "foo.rs");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "TODO");
        assert_eq!(violations[0].line, 2);
    }

    #[test]
    fn scan_content_finds_unimplemented() {
        let content = "fn bar() -> i32 {\n    unimplemented!()\n}\n";
        let violations = scan_content(content, "bar.rs");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "unimplemented!()");
    }

    #[test]
    fn scan_patch_only_added_lines() {
        let patch = "\
--- a/old.rs
+++ b/new.rs
@@ -1,3 +1,4 @@
 fn keep() {}
-fn old() {}
+fn new() {
+    // TODO: wire this up
+}
";
        let violations = scan_patch(patch);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pattern, "TODO");
    }

    #[test]
    fn scan_content_clean() {
        let content = "fn clean() -> bool { true }\n";
        let violations = scan_content(content, "clean.rs");
        assert!(violations.is_empty());
    }
}
