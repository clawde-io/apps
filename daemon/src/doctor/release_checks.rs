// SPDX-License-Identifier: MIT
//! Release lock enforcement checks for `clawd doctor`.
//!
//! D64.T15-T18:
//!   T15 — git pre-tag hook installation check
//!   T16 — version bump detection (scan only — file-watch is future work)
//!   T17 — release plan validation (.claude/planning/release-{version}.md)
//!   T18 — approveRelease (implemented in mod.rs::approve_release)

use super::{DoctorFinding, DoctorSeverity};
use std::path::Path;

pub fn run(project_path: &Path) -> Vec<DoctorFinding> {
    let mut findings = Vec::new();

    // T15 — check if pre-tag hook is installed
    let pre_tag_hook = project_path.join(".git/hooks/pre-tag");
    let git_dir = project_path.join(".git");
    if git_dir.exists() && !pre_tag_hook.exists() {
        findings.push(DoctorFinding {
            code: "release.missing_pre_tag_hook".to_string(),
            severity: DoctorSeverity::Medium,
            message: "Git pre-tag hook not installed. Run `clawd hook install` to prevent unplanned version tags.".to_string(),
            path: Some(".git/hooks/pre-tag".to_string()),
            fixable: false,
        });
    }

    // T17 — check release plans for required sections
    let planning_dir = project_path.join(".claude/planning");
    if planning_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&planning_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("release-") || !name.ends_with(".md") {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    let issues = validate_release_plan(&content);
                    if !issues.is_empty() {
                        findings.push(DoctorFinding {
                            code: "release.incomplete_plan".to_string(),
                            severity: DoctorSeverity::Medium,
                            message: format!(
                                "Release plan {} is missing required sections: {}",
                                name,
                                issues.join(", ")
                            ),
                            path: Some(format!(".claude/planning/{name}")),
                            fixable: false,
                        });
                    }
                }
            }
        }
    }

    findings
}

/// Check that a release plan has all required sections.
/// Returns a list of missing section names.
fn validate_release_plan(content: &str) -> Vec<String> {
    let required_sections = [
        "Version",
        "Features",
        "Migration",
        "Rollback",
        "Deployment",
        "Status",
    ];
    required_sections
        .iter()
        .filter(|&&s| {
            !content.contains(&format!("## {s}"))
                && !content.contains(&format!("**{s}**"))
        })
        .map(|s| s.to_string())
        .collect()
}

/// Install the git pre-tag hook that blocks unplanned version tags.
/// Called by `doctor.hookInstall` RPC and `clawd hook install` CLI.
pub fn install_pre_tag_hook(project_path: &Path) -> std::io::Result<()> {
    let hooks_dir = project_path.join(".git/hooks");
    if !hooks_dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not a git repository (no .git/hooks/)",
        ));
    }

    let hook_path = hooks_dir.join("pre-tag");
    let hook_content = r#"#!/usr/bin/env bash
# clawd release lock — installed by `clawd hook install`
# Blocks `git tag v*` unless a FORGE-approved release plan exists.

TAG="$1"

# Only gate version tags (v0.1.0, v1.2.3, etc.)
if [[ ! "$TAG" =~ ^v[0-9] ]]; then
    exit 0
fi

PLANNING_DIR=".claude/planning"
RELEASE_PLAN="${PLANNING_DIR}/release-${TAG}.md"

if [ ! -f "$RELEASE_PLAN" ]; then
    echo ""
    echo "  clawd release lock: blocked."
    echo "  No release plan found for ${TAG}."
    echo ""
    echo "  Create .claude/planning/release-${TAG}.md with a FORGE-approved"
    echo "  release plan before tagging. Run \`clawd doctor\` to check."
    echo ""
    exit 1
fi

# Check for approved status in the release plan
if ! grep -qi "status.*approved\|approved.*status" "$RELEASE_PLAN"; then
    echo ""
    echo "  clawd release lock: blocked."
    echo "  Release plan for ${TAG} exists but is not approved."
    echo ""
    echo "  Run \`clawd doctor.approveRelease ${TAG}\` or edit"
    echo "  ${RELEASE_PLAN} to set Status: approved."
    echo ""
    exit 1
fi

exit 0
"#;

    std::fs::write(&hook_path, hook_content)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms)?;
    }

    Ok(())
}
