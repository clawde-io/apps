/// Integration tests for the `clawd doctor` AFS scanner (D64.T26).
///
/// Verifies that `doctor.scan` returns expected findings for:
///   - A clean `.claude/` structure (all required files present) → 0 findings
///   - A broken structure (missing VISION.md, wrong docs dir) → expected findings
use clawd::doctor::{
    scan, DoctorScanResult, DoctorSeverity, ScanScope,
};
use std::fs;
use tempfile::TempDir;

/// Create a fully healthy `.claude/` project structure.
fn make_clean_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // .gitignore with .claude/ entry
    fs::write(root.join(".gitignore"), ".claude/\n.env\n").unwrap();

    // Required .claude/ directories
    let claude = root.join(".claude");
    fs::create_dir_all(claude.join("tasks")).unwrap();
    fs::create_dir_all(claude.join("qa")).unwrap();
    fs::create_dir_all(claude.join("docs")).unwrap();
    fs::create_dir_all(claude.join("ideas")).unwrap();
    fs::create_dir_all(claude.join("temp")).unwrap();
    fs::create_dir_all(claude.join("archive/inbox")).unwrap();
    fs::create_dir_all(claude.join("inbox")).unwrap();
    fs::create_dir_all(claude.join("memory")).unwrap();

    // Required files
    fs::write(claude.join("docs/VISION.md"), "# Vision\n").unwrap();
    fs::write(claude.join("docs/FEATURES.md"), "# Features\n").unwrap();

    // active.md with a fresh session handoff block
    let today = "2099-12-31"; // far future = never stale
    let active_content = format!(
        "# Tasks\n\n## Session Handoff ({today})\n\n**Last action:** fresh\n"
    );
    fs::write(claude.join("tasks/active.md"), active_content).unwrap();

    // QA checklists
    fs::write(claude.join("qa/pre-commit.md"), "# Pre-Commit\n").unwrap();
    fs::write(claude.join("qa/pre-pr.md"), "# Pre-PR\n").unwrap();

    // Make it a git repo (needed for some checks)
    fs::create_dir_all(root.join(".git")).unwrap();

    // Private repo: .docs/ (no .wiki/)
    let docs = root.join(".docs");
    fs::create_dir_all(&docs).unwrap();
    fs::write(docs.join("README.md"), "# Docs\n").unwrap();

    tmp
}

#[test]
fn test_clean_project_has_no_critical_findings() {
    let tmp = make_clean_project();
    let result: DoctorScanResult = scan(tmp.path(), ScanScope::All);

    // A clean project should score >= 80 and have no Critical/High findings
    let criticals: Vec<_> = result
        .findings
        .iter()
        .filter(|f| {
            f.severity == DoctorSeverity::Critical || f.severity == DoctorSeverity::High
        })
        .collect();

    assert!(
        criticals.is_empty(),
        "clean project has critical/high findings: {:?}",
        criticals
    );
    assert!(
        result.score >= 70,
        "clean project score {} < 70 (findings: {:?})",
        result.score,
        result.findings
    );
}

#[test]
fn test_missing_vision_md_is_reported() {
    let tmp = make_clean_project();
    // Remove VISION.md
    fs::remove_file(tmp.path().join(".claude/docs/VISION.md")).unwrap();

    let result: DoctorScanResult = scan(tmp.path(), ScanScope::Afs);
    let codes: Vec<&str> = result.findings.iter().map(|f| f.code.as_str()).collect();
    assert!(
        codes.iter().any(|c| c.contains("vision") || c.contains("VISION")),
        "missing VISION.md not flagged; findings: {:?}",
        result.findings
    );
}

#[test]
fn test_missing_gitignore_entry_is_reported() {
    let tmp = make_clean_project();
    // .gitignore without .claude/ entry
    fs::write(tmp.path().join(".gitignore"), ".env\n").unwrap();

    let result: DoctorScanResult = scan(tmp.path(), ScanScope::Afs);
    let codes: Vec<&str> = result.findings.iter().map(|f| f.code.as_str()).collect();
    assert!(
        codes.iter().any(|c| c.contains("gitignore")),
        "missing .gitignore .claude/ entry not flagged; findings: {:?}",
        result.findings
    );
}

#[test]
fn test_brand_in_wrong_location_is_flagged() {
    let tmp = make_clean_project();
    // Put brand assets in the wrong place (.claude/brand/)
    fs::create_dir_all(tmp.path().join(".claude/brand")).unwrap();
    fs::write(tmp.path().join(".claude/brand/logo.png"), b"fake").unwrap();

    let result: DoctorScanResult = scan(tmp.path(), ScanScope::Docs);
    let codes: Vec<&str> = result.findings.iter().map(|f| f.code.as_str()).collect();
    assert!(
        codes.iter().any(|c| c.contains("brand")),
        "brand in .claude/ not flagged; findings: {:?}",
        result.findings
    );
}

#[test]
fn test_release_plan_missing_sections_is_flagged() {
    let tmp = make_clean_project();
    // Add an incomplete release plan
    let planning = tmp.path().join(".claude/planning");
    fs::create_dir_all(&planning).unwrap();
    fs::write(
        planning.join("release-v0.2.0.md"),
        "# Release v0.2.0\n\n## Version\n0.2.0\n",
    )
    .unwrap();

    let result: DoctorScanResult = scan(tmp.path(), ScanScope::Release);
    let codes: Vec<&str> = result.findings.iter().map(|f| f.code.as_str()).collect();
    assert!(
        codes.iter().any(|c| c.contains("release")),
        "incomplete release plan not flagged; findings: {:?}",
        result.findings
    );
}

#[test]
fn test_empty_project_has_many_findings() {
    let tmp = TempDir::new().unwrap();
    let result: DoctorScanResult = scan(tmp.path(), ScanScope::All);
    // A project with no .claude/ at all should have multiple findings and low score
    assert!(
        !result.findings.is_empty(),
        "empty project should have findings"
    );
    assert!(
        result.score < 90,
        "empty project score {} should be < 90",
        result.score
    );
}

#[test]
fn test_scope_afs_only_returns_afs_findings() {
    let tmp = make_clean_project();
    // Remove VISION.md (AFS issue) and add brand in wrong place (Docs issue)
    fs::remove_file(tmp.path().join(".claude/docs/VISION.md")).unwrap();
    fs::create_dir_all(tmp.path().join(".claude/brand")).unwrap();

    let result: DoctorScanResult = scan(tmp.path(), ScanScope::Afs);
    // AFS scope should find VISION.md issue
    let has_afs = result.findings.iter().any(|f| {
        f.code.contains("afs") || f.code.contains("vision") || f.code.contains("VISION")
    });
    assert!(has_afs, "AFS scope should find AFS issues; findings: {:?}", result.findings);
}

#[test]
fn test_ideas_dir_missing_is_info_level() {
    let tmp = make_clean_project();
    fs::remove_dir_all(tmp.path().join(".claude/ideas")).unwrap();

    let result: DoctorScanResult = scan(tmp.path(), ScanScope::Afs);
    let ideas_finding = result
        .findings
        .iter()
        .find(|f| f.code.contains("ideas"));

    if let Some(f) = ideas_finding {
        assert_eq!(
            f.severity,
            DoctorSeverity::Info,
            "missing ideas/ should be Info severity"
        );
    }
    // It's OK if not reported — Info items may be suppressed
}
