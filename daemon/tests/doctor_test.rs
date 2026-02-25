// SPDX-License-Identifier: MIT
//! Integration tests for clawd::doctor::scan (D64.T26).

use clawd::doctor::{scan, DoctorSeverity, ScanScope};
use std::fs;
use tempfile::TempDir;

/// Helper: create the full valid AFS structure inside a temp dir.
fn scaffold_healthy(root: &std::path::Path) {
    // .claude/ required directories
    for dir in &[
        ".claude/docs",
        ".claude/tasks",
        ".claude/qa",
        ".claude/ideas",
        ".claude/memory",
        ".claude/planning",
        ".claude/temp",
    ] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }

    // Required files per afs_checks T04
    fs::write(
        root.join(".claude/docs/VISION.md"),
        "# Vision
",
    )
    .unwrap();
    fs::write(
        root.join(".claude/docs/FEATURES.md"),
        "# Features
",
    )
    .unwrap();
    fs::write(
        root.join(".claude/tasks/active.md"),
        "# Active
",
    )
    .unwrap();
    fs::write(
        root.join(".claude/qa/pre-commit.md"),
        "# Pre-commit
",
    )
    .unwrap();
    fs::write(
        root.join(".claude/qa/pre-pr.md"),
        "# Pre-PR
",
    )
    .unwrap();

    // .gitignore with .claude/ entry (afs_checks T05)
    fs::write(
        root.join(".gitignore"),
        ".claude/
.DS_Store
",
    )
    .unwrap();
}

/// D64.T26 — healthy project scores >= 90 with no Critical or High findings.
#[test]
fn test_doctor_scan_healthy() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    scaffold_healthy(root);

    let result = scan(root, ScanScope::All);

    assert!(
        result.score >= 90,
        "healthy project should score >= 90, got {}. Findings: {:#?}",
        result.score,
        result.findings
    );

    let critical_or_high: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.severity == DoctorSeverity::Critical || f.severity == DoctorSeverity::High)
        .collect();

    assert!(
        critical_or_high.is_empty(),
        "healthy project should have no Critical or High findings, got: {:#?}",
        critical_or_high
    );
}

/// D64.T26 — unhealthy project scores < 100, has findings, and has at least one Critical/High.
#[test]
fn test_doctor_scan_unhealthy() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create bare .claude/ so the dir exists (avoids the single afs.missing_claude_dir bail-out)
    // but deliberately omit VISION.md and active.md.
    fs::create_dir_all(root.join(".claude/docs")).unwrap();
    fs::create_dir_all(root.join(".claude/tasks")).unwrap();
    fs::create_dir_all(root.join(".claude/qa")).unwrap();
    fs::create_dir_all(root.join(".claude/ideas")).unwrap();

    // Present: FEATURES.md, pre-commit.md, pre-pr.md -- but NOT VISION.md or active.md
    fs::write(
        root.join(".claude/docs/FEATURES.md"),
        "# Features
",
    )
    .unwrap();
    fs::write(
        root.join(".claude/qa/pre-commit.md"),
        "# Pre-commit
",
    )
    .unwrap();
    fs::write(
        root.join(".claude/qa/pre-pr.md"),
        "# Pre-PR
",
    )
    .unwrap();
    // Deliberately missing: .claude/docs/VISION.md  (afs.missing_vision -- High)
    // Deliberately missing: .claude/tasks/active.md (afs.missing_active_md -- Critical)

    // Mutual exclusivity violation: both .docs/ and .wiki/ present
    // (docs.both_docs_and_wiki -- High)
    fs::create_dir_all(root.join(".docs")).unwrap();
    fs::write(
        root.join(".docs/README.md"),
        "# Docs
",
    )
    .unwrap();
    fs::create_dir_all(root.join(".wiki")).unwrap();
    fs::write(
        root.join(".wiki/Home.md"),
        "# Wiki
",
    )
    .unwrap();

    let result = scan(root, ScanScope::All);

    assert!(
        result.score < 100,
        "unhealthy project should score < 100, got {}",
        result.score
    );

    assert!(
        !result.findings.is_empty(),
        "unhealthy project should have at least one finding"
    );

    let has_critical_or_high = result
        .findings
        .iter()
        .any(|f| f.severity == DoctorSeverity::Critical || f.severity == DoctorSeverity::High);

    assert!(
        has_critical_or_high,
        "unhealthy project should have at least one Critical or High finding. Findings: {:#?}",
        result.findings
    );
}
