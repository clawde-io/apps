// SPDX-License-Identifier: MIT
//! AFS (Agent File System) health checks for `clawd doctor`.
//!
//! D64.T04-T09:
//!   T04 — required files check (VISION.md, FEATURES.md, active.md, pre-commit.md, pre-pr.md)
//!   T05 — .gitignore includes .claude/
//!   T06 — active.md staleness (last modified within 7 days)
//!   T07 — ideas/ directory exists
//!   T08 — temp/ stale files (older than 24h)
//!   T09 — AFS scoring (handled in mod.rs via penalty())

use super::{DoctorFinding, DoctorSeverity};
use std::path::Path;

pub fn run(project_path: &Path) -> Vec<DoctorFinding> {
    let mut findings = Vec::new();
    let claude = project_path.join(".claude");

    if !claude.exists() {
        // No .claude/ at all — single critical finding, skip sub-checks
        findings.push(DoctorFinding {
            code: "afs.missing_claude_dir".to_string(),
            severity: DoctorSeverity::Critical,
            message: "No .claude/ directory found. Run `clawd init` to scaffold the AFS structure."
                .to_string(),
            path: Some(".claude/".to_string()),
            fixable: false,
        });
        return findings;
    }

    // T04 — required files
    let required = [
        (
            ".claude/docs/VISION.md",
            DoctorSeverity::High,
            "afs.missing_vision",
        ),
        (
            ".claude/docs/FEATURES.md",
            DoctorSeverity::High,
            "afs.missing_features",
        ),
        (
            ".claude/tasks/active.md",
            DoctorSeverity::Critical,
            "afs.missing_active_md",
        ),
        (
            ".claude/qa/pre-commit.md",
            DoctorSeverity::Medium,
            "afs.missing_pre_commit",
        ),
        (
            ".claude/qa/pre-pr.md",
            DoctorSeverity::Medium,
            "afs.missing_pre_pr",
        ),
    ];

    for (rel_path, severity, code) in &required {
        if !project_path.join(rel_path).exists() {
            findings.push(DoctorFinding {
                code: code.to_string(),
                severity: severity.clone(),
                message: format!("Required file missing: {rel_path}"),
                path: Some(rel_path.to_string()),
                fixable: false,
            });
        }
    }

    // T05 — .gitignore includes .claude/
    let gitignore = project_path.join(".gitignore");
    if gitignore.exists() {
        let content = std::fs::read_to_string(&gitignore).unwrap_or_default();
        if !content.contains(".claude/") && !content.contains(".claude") {
            findings.push(DoctorFinding {
                code: "afs.missing_gitignore_entry".to_string(),
                severity: DoctorSeverity::Medium,
                message: ".gitignore does not include .claude/ — AI working memory may be accidentally committed.".to_string(),
                path: Some(".gitignore".to_string()),
                fixable: true,
            });
        }
    }

    // T06 — active.md staleness (last modified within 7 days)
    let active_md = project_path.join(".claude/tasks/active.md");
    if active_md.exists() {
        if let Ok(meta) = std::fs::metadata(&active_md) {
            if let Ok(modified) = meta.modified() {
                let age = std::time::SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default();
                if age.as_secs() > 7 * 24 * 60 * 60 {
                    findings.push(DoctorFinding {
                        code: "afs.stale_active_md".to_string(),
                        severity: DoctorSeverity::Low,
                        message: format!(
                            "active.md has not been updated in {} days. Add a Session Handoff block.",
                            age.as_secs() / (24 * 60 * 60)
                        ),
                        path: Some(".claude/tasks/active.md".to_string()),
                        fixable: false,
                    });
                }
            }
        }
    }

    // T07 — ideas/ directory exists
    let ideas = claude.join("ideas");
    if !ideas.exists() {
        findings.push(DoctorFinding {
            code: "afs.missing_ideas_dir".to_string(),
            severity: DoctorSeverity::Info,
            message: ".claude/ideas/ directory not found. Create it to capture ideas.".to_string(),
            path: Some(".claude/ideas/".to_string()),
            fixable: true,
        });
    }

    // T08 — temp/ stale files (older than 24h)
    let temp = claude.join("temp");
    if temp.exists() {
        let cutoff = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(24 * 60 * 60))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let stale_count = std::fs::read_dir(&temp)
            .ok()
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| {
                        e.metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .map(|t| t < cutoff)
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0);
        if stale_count > 0 {
            findings.push(DoctorFinding {
                code: "afs.stale_temp".to_string(),
                severity: DoctorSeverity::Low,
                message: format!(
                    "{stale_count} stale file(s) in .claude/temp/ (older than 24h)."
                ),
                path: Some(".claude/temp/".to_string()),
                fixable: true,
            });
        }
    }

    findings
}
