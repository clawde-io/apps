//! Sprint CC GD.7 — Ghost Diff engine tests.

use std::io::Write;
use tempfile::TempDir;

fn write_file(dir: &TempDir, name: &str, content: &str) {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
}

#[test]
fn spec_parser_extracts_expected_behaviors() {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "session.md",
        "# Session Spec\n\n## Expected behavior\n\n- Sessions must persist across restarts\n- Session IDs must be unique\n- Timeout after 30 minutes idle\n",
    );

    let specs = clawd::ghost_diff::spec_parser::load_specs(dir.path()).unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].name, "session.md");
    assert!(!specs[0].expected_behaviors.is_empty());
}

#[test]
fn spec_parser_empty_dir_returns_empty() {
    let dir = TempDir::new().unwrap();
    let specs = clawd::ghost_diff::spec_parser::load_specs(dir.path()).unwrap();
    assert!(specs.is_empty());
}

#[test]
fn spec_parser_nonexistent_dir_returns_empty() {
    let specs = clawd::ghost_diff::spec_parser::load_specs(
        std::path::Path::new("/nonexistent/path/to/specs"),
    )
    .unwrap();
    assert!(specs.is_empty());
}

#[tokio::test]
async fn ghost_diff_skips_when_no_specs_dir() {
    let dir = TempDir::new().unwrap();
    // No .claw/specs/ directory — should return empty warnings.
    let warnings =
        clawd::ghost_diff::engine::check_ghost_drift(dir.path().to_str().unwrap(), None)
            .await
            .unwrap();
    assert!(warnings.is_empty(), "no specs = no warnings");
}

#[tokio::test]
async fn ghost_diff_no_false_positive_on_matching_code() {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        ".claw/specs/session.md",
        "## Expected behavior\n- Sessions persist using SQLite storage\n",
    );
    // Write a file that mentions "session" and "sqlite" and "storage" — matches spec.
    write_file(
        &dir,
        "src/session.rs",
        "// Session management using SQLite storage\npub struct Session { id: String }",
    );

    // No git diff in temp dir, so changed_files will be empty — should return no warnings.
    let warnings =
        clawd::ghost_diff::engine::check_ghost_drift(dir.path().to_str().unwrap(), None)
            .await
            .unwrap();
    assert!(warnings.is_empty(), "no git diff = no changed files = no warnings");
}
