// SPDX-License-Identifier: MIT
//! .docs/.wiki enforcement checks for `clawd doctor`.
//!
//! D64.T10-T14:
//!   T10 — check .docs/README.md exists when .docs/ is present
//!   T11 — .docs/.wiki mutual exclusivity
//!   T12 — brand assets in wrong location
//!   T13 — .claude/ file count > 40 (lean-files rule)
//!   T14 — auto-fix stubs (create .docs/README.md, create ideas/ dir, clean old temp)

use super::{DoctorFinding, DoctorSeverity};
use std::path::Path;

pub fn run(project_path: &Path) -> Vec<DoctorFinding> {
    let mut findings = Vec::new();

    // T11 — .docs/.wiki mutual exclusivity check
    let has_docs = project_path.join(".docs").exists();
    let has_wiki = project_path.join(".wiki").exists();

    if has_docs && has_wiki {
        findings.push(DoctorFinding {
            code: "docs.both_docs_and_wiki".to_string(),
            severity: DoctorSeverity::High,
            message: "Both .docs/ and .wiki/ exist. Private repos use .docs/ only; public repos use .wiki/ only. Remove one.".to_string(),
            path: None,
            fixable: false,
        });
    }

    // T12 — brand assets in wrong location
    let wrong_brand_paths = [
        ".claude/brand",
        ".claude/branding",
        ".brand",
        "src/assets/brand",
    ];
    for wrong_path in &wrong_brand_paths {
        if project_path.join(wrong_path).exists() {
            findings.push(DoctorFinding {
                code: "docs.brand_in_wrong_location".to_string(),
                severity: DoctorSeverity::Medium,
                message: format!(
                    "Brand assets found at {wrong_path}. Move to .docs/brand/ (private repo) or project root .docs/brand/ (multi-repo)."
                ),
                path: Some(wrong_path.to_string()),
                fixable: false,
            });
        }
    }

    // T13 — .claude/ file count > 40
    let claude = project_path.join(".claude");
    if claude.exists() {
        let file_count = count_files_recursive(&claude);
        if file_count > 40 {
            findings.push(DoctorFinding {
                code: "docs.too_many_claude_files".to_string(),
                severity: DoctorSeverity::Low,
                message: format!(
                    ".claude/ has {file_count} files (limit: 40). Archive done tasks and prune stale planning docs."
                ),
                path: Some(".claude/".to_string()),
                fixable: false,
            });
        }
    }

    // T10 — check that .docs/README.md exists if .docs/ is present
    let docs_readme = project_path.join(".docs/README.md");
    if has_docs && !docs_readme.exists() {
        findings.push(DoctorFinding {
            code: "docs.missing_docs_readme".to_string(),
            severity: DoctorSeverity::Low,
            message: ".docs/ exists but is missing README.md (required index file).".to_string(),
            path: Some(".docs/README.md".to_string()),
            fixable: true,
        });
    }

    findings
}

/// Count all files (not directories) under a path recursively.
fn count_files_recursive(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut count = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            count += count_files_recursive(&path);
        } else {
            count += 1;
        }
    }
    count
}
