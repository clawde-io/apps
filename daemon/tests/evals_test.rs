//! Integration tests for the evals module.
//!
//! Covers the three CRITICAL/HIGH scanners:
//!   - Placeholder detector
//!   - Secrets scanner
//!   - Forbidden tool detector

use clawd::evals::scanners::{
    forbidden::check_tool_allowed,
    placeholders::{scan_content, scan_patch},
    secrets::scan_patch as secret_scan_patch,
};

// ─── Placeholder detector ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_placeholder_detector_finds_todo_in_content() {
    let content = "fn process() {\n    // TODO: implement me\n    println!(\"stub\");\n}\n";
    let violations = scan_content(content, "process.rs");
    assert!(
        !violations.is_empty(),
        "Expected TODO to be flagged as placeholder"
    );
    assert_eq!(violations[0].pattern, "TODO");
    assert_eq!(violations[0].line, 2);
    assert_eq!(violations[0].file, "process.rs");
}

#[tokio::test]
async fn test_placeholder_detector_finds_unimplemented_macro() {
    let content = "fn stub_fn() -> String {\n    unimplemented!()\n}\n";
    let violations = scan_content(content, "stub.rs");
    assert!(!violations.is_empty());
    assert_eq!(violations[0].pattern, "unimplemented!()");
}

#[tokio::test]
async fn test_placeholder_detector_finds_todo_macro() {
    let content = "fn work() -> u32 {\n    todo!()\n}\n";
    let violations = scan_content(content, "work.rs");
    assert!(!violations.is_empty());
    assert_eq!(violations[0].pattern, "todo!()");
}

#[tokio::test]
async fn test_placeholder_detector_clean_code_passes() {
    let content = "fn clean() -> bool {\n    true\n}\n";
    let violations = scan_content(content, "clean.rs");
    assert!(
        violations.is_empty(),
        "Clean code should have no placeholder violations"
    );
}

#[tokio::test]
async fn test_placeholder_patch_only_added_lines() {
    // Removed line has TODO but should NOT be flagged — only added lines count.
    let patch = "\
--- a/old.rs
+++ b/new.rs
@@ -1,2 +1,2 @@
-// TODO: old comment
+// This is the new comment
 fn foo() {}
";
    let violations = scan_patch(patch);
    assert!(
        violations.is_empty(),
        "Removed TODO line should not trigger violation"
    );
}

// ─── Secrets scanner ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_secrets_scanner_finds_api_key() {
    let patch = "\
--- a/config.rs
+++ b/config.rs
@@ -1,1 +1,2 @@
 fn init() {}
+const API_KEY: &str = \"sk-abcdefghijklmnopqrstuvwxyz1234567890\";
";
    let violations = secret_scan_patch(patch);
    assert!(
        !violations.is_empty(),
        "OpenAI/Anthropic key should be detected"
    );
    assert_eq!(violations[0].secret_type, "openai_key");
}

#[tokio::test]
async fn test_secrets_scanner_finds_github_token() {
    let patch = "\
--- a/ci.yml
+++ b/ci.yml
@@ -1,1 +1,2 @@
 run: echo ok
+TOKEN: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij
";
    let violations = secret_scan_patch(patch);
    assert!(!violations.is_empty(), "GitHub token should be detected");
    assert_eq!(violations[0].secret_type, "github_token");
}

#[tokio::test]
async fn test_secrets_scanner_finds_aws_key() {
    let patch = "\
--- a/aws.rs
+++ b/aws.rs
@@ -1,1 +1,2 @@
 fn setup() {}
+let key = \"AKIAIOSFODNN7EXAMPLE\";
";
    let violations = secret_scan_patch(patch);
    assert!(!violations.is_empty(), "AWS access key should be detected");
    assert_eq!(violations[0].secret_type, "aws_key");
}

#[tokio::test]
async fn test_secrets_scanner_clean_diff_no_violations() {
    let patch = "\
--- a/main.rs
+++ b/main.rs
@@ -1,1 +1,2 @@
 fn main() {}
+fn helper() { println!(\"hello world\"); }
";
    let violations = secret_scan_patch(patch);
    assert!(
        violations.is_empty(),
        "Clean diff should have no secret violations"
    );
}

#[tokio::test]
async fn test_secrets_scanner_ignores_removed_lines() {
    // API key is on a removed line — should not be flagged.
    let patch = "\
--- a/config.rs
+++ b/config.rs
@@ -1,2 +1,1 @@
-const KEY: &str = \"sk-abcdefghijklmnopqrstuvwxyz1234567890\";
+const KEY: &str = \"\";
";
    let violations = secret_scan_patch(patch);
    assert!(
        violations.is_empty(),
        "Key on removed line should not be flagged"
    );
}

// ─── Forbidden tool detector ──────────────────────────────────────────────────

#[tokio::test]
async fn test_forbidden_tool_network_without_permission() {
    let violation = check_tool_allowed("http_get", &[], None, None);
    assert!(
        violation.is_some(),
        "http_get without 'network' must be blocked"
    );
    let v = violation.unwrap();
    assert_eq!(v.permission_needed, "network");
    assert_eq!(v.tool_name, "http_get");
}

#[tokio::test]
async fn test_forbidden_tool_network_with_permission_allowed() {
    let perms = vec!["network".to_string()];
    let violation = check_tool_allowed("http_get", &perms, None, None);
    assert!(
        violation.is_none(),
        "http_get with 'network' should be allowed"
    );
}

#[tokio::test]
async fn test_forbidden_tool_shell_without_permission() {
    let violation = check_tool_allowed("bash", &[], None, None);
    assert!(
        violation.is_some(),
        "bash without 'shell_exec' must be blocked"
    );
    assert_eq!(violation.unwrap().permission_needed, "shell_exec");
}

#[tokio::test]
async fn test_forbidden_tool_shell_with_permission_allowed() {
    let perms = vec!["shell_exec".to_string()];
    let violation = check_tool_allowed("bash", &perms, None, None);
    assert!(violation.is_none());
}

#[tokio::test]
async fn test_forbidden_tool_git_push_without_permission() {
    let violation = check_tool_allowed("git_push", &[], None, None);
    assert!(violation.is_some());
    assert_eq!(violation.unwrap().permission_needed, "git");
}

#[tokio::test]
async fn test_forbidden_tool_write_outside_worktree() {
    let perms = vec!["write".to_string()];
    let violation = check_tool_allowed(
        "write_file",
        &perms,
        Some("/etc/cron.d/malicious"),
        Some("/Users/user/myproject"),
    );
    assert!(
        violation.is_some(),
        "Write outside worktree must be blocked"
    );
    assert!(violation.unwrap().reason.contains("outside the worktree"));
}

#[tokio::test]
async fn test_forbidden_tool_write_inside_worktree_allowed() {
    let perms = vec!["write".to_string()];
    let violation = check_tool_allowed(
        "write_file",
        &perms,
        Some("/Users/user/myproject/src/lib.rs"),
        Some("/Users/user/myproject"),
    );
    assert!(
        violation.is_none(),
        "Write inside worktree should be allowed"
    );
}

#[tokio::test]
async fn test_forbidden_tool_unknown_tool_allowed() {
    let violation = check_tool_allowed("read_file", &[], None, None);
    assert!(
        violation.is_none(),
        "Unknown/read-only tools should be allowed by default"
    );
}
