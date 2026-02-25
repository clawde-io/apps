/// Repo intelligence drift detection — artifact staleness and drift scoring (RI.T14–T16).
///
/// Distinct from the feature-drift scanner in `crate::drift` (which detects features
/// in FEATURES.md with no implementation). This module detects config-level drift:
/// CLAUDE.md / AGENTS.md / .cursor/rules being out of sync with the actual repo stack.
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{Duration, SystemTime};

// ─── Types ────────────────────────────────────────────────────────────────────

/// A detected repo-level drift issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoDriftItem {
    pub kind: RepoDriftKind,
    pub message: String,
    pub path: Option<String>,
    pub severity: RepoDriftSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepoDriftKind {
    /// An AI artifact file is missing (.claude/CLAUDE.md, .codex/AGENTS.md, etc.)
    MissingArtifact,
    /// An AI artifact is older than 30 days relative to config files
    StaleArtifact,
    /// A key config file changed but the artifact was not updated
    ConfigChangedArtifactStale,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum RepoDriftSeverity {
    Low,
    Medium,
    High,
}

// ─── Drift score ──────────────────────────────────────────────────────────────

/// Compute a 0–100 drift score for a repository (RI.T15).
///
/// 100 = perfectly in sync; lower = more drift detected.
///
/// Scoring:
///  - CLAUDE.md missing → -40
///  - AGENTS.md missing → -20
///  - .cursor/rules missing → -20
///  - Any artifact older than config by >30 days → -10 each
pub fn drift_score(repo_path: &Path) -> u8 {
    let items = drift_report_internal(repo_path);
    let penalty: i32 = items
        .iter()
        .map(|item| match item.severity {
            RepoDriftSeverity::High => 40,
            RepoDriftSeverity::Medium => 20,
            RepoDriftSeverity::Low => 10,
        })
        .sum();
    let score = 100i32 - penalty;
    score.max(0) as u8
}

/// Return a list of detected repo drift issues (RI.T16).
pub fn drift_report(repo_path: &Path) -> Vec<RepoDriftItem> {
    drift_report_internal(repo_path)
}

fn drift_report_internal(repo_path: &Path) -> Vec<RepoDriftItem> {
    let mut items = Vec::new();

    // Check artifact presence
    let claude_path = repo_path.join(".claude").join("CLAUDE.md");
    let agents_path = repo_path.join(".codex").join("AGENTS.md");
    let cursor_path = repo_path.join(".cursor").join("rules");

    if !claude_path.exists() {
        items.push(RepoDriftItem {
            kind: RepoDriftKind::MissingArtifact,
            message: ".claude/CLAUDE.md is missing — run repo.generateArtifacts to create it"
                .to_string(),
            path: Some(claude_path.to_string_lossy().into_owned()),
            severity: RepoDriftSeverity::High,
        });
    }
    if !agents_path.exists() {
        items.push(RepoDriftItem {
            kind: RepoDriftKind::MissingArtifact,
            message: ".codex/AGENTS.md is missing — run repo.generateArtifacts to create it"
                .to_string(),
            path: Some(agents_path.to_string_lossy().into_owned()),
            severity: RepoDriftSeverity::Medium,
        });
    }
    if !cursor_path.exists() {
        items.push(RepoDriftItem {
            kind: RepoDriftKind::MissingArtifact,
            message: ".cursor/rules is missing — run repo.generateArtifacts to create it"
                .to_string(),
            path: Some(cursor_path.to_string_lossy().into_owned()),
            severity: RepoDriftSeverity::Medium,
        });
    }

    // Check staleness: if a config manifest changed more recently than the artifact, flag it
    let config_candidates = [
        repo_path.join("Cargo.toml"),
        repo_path.join("package.json"),
        repo_path.join("pubspec.yaml"),
        repo_path.join("go.mod"),
        repo_path.join("pyproject.toml"),
        repo_path.join("tsconfig.json"),
    ];
    let latest_config_mtime = config_candidates.iter().filter_map(|p| mtime(p)).max();

    if let Some(config_mtime) = latest_config_mtime {
        for (artifact, name) in &[
            (&claude_path, ".claude/CLAUDE.md"),
            (&agents_path, ".codex/AGENTS.md"),
            (&cursor_path, ".cursor/rules"),
        ] {
            if let Some(artifact_mtime) = mtime(artifact) {
                // If artifact is more than 30 days older than the config, flag it
                if config_mtime
                    .duration_since(artifact_mtime)
                    .unwrap_or(Duration::ZERO)
                    > Duration::from_secs(30 * 24 * 3600)
                {
                    items.push(RepoDriftItem {
                        kind: RepoDriftKind::ConfigChangedArtifactStale,
                        message: format!(
                            "{name} is more than 30 days older than the project config — consider regenerating"
                        ),
                        path: Some(artifact.to_string_lossy().into_owned()),
                        severity: RepoDriftSeverity::Low,
                    });
                }
            }
        }
    }

    items
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

// ─── Staleness watcher (RI.T14) ───────────────────────────────────────────────

/// Check if any artifact is stale relative to config files.
///
/// Returns true if the caller should trigger a re-scan (e.g. re-compute drift score).
pub fn check_staleness(repo_path: &Path) -> bool {
    drift_score(repo_path) < 80
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn empty_repo_has_low_score() {
        let tmp = TempDir::new().unwrap();
        let score = drift_score(tmp.path());
        assert!(
            score < 60,
            "expected low score for repo with no artifacts, got {score}"
        );
    }

    #[test]
    fn repo_with_all_artifacts_scores_100() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".claude")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".codex")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".cursor")).unwrap();
        std::fs::write(tmp.path().join(".claude").join("CLAUDE.md"), b"# ok\n").unwrap();
        std::fs::write(tmp.path().join(".codex").join("AGENTS.md"), b"# ok\n").unwrap();
        std::fs::write(tmp.path().join(".cursor").join("rules"), b"ok\n").unwrap();
        let score = drift_score(tmp.path());
        assert_eq!(score, 100);
    }

    #[test]
    fn missing_claude_md_lowers_score() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".codex")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".cursor")).unwrap();
        std::fs::write(tmp.path().join(".codex").join("AGENTS.md"), b"# ok\n").unwrap();
        std::fs::write(tmp.path().join(".cursor").join("rules"), b"ok\n").unwrap();
        // No .claude/CLAUDE.md
        let score = drift_score(tmp.path());
        assert!(score < 80, "expected score < 80, got {score}");
    }

    #[test]
    fn drift_report_lists_missing_artifacts() {
        let tmp = TempDir::new().unwrap();
        let report = drift_report(tmp.path());
        let kinds: Vec<_> = report.iter().map(|i| &i.kind).collect();
        assert!(matches!(kinds[0], RepoDriftKind::MissingArtifact));
    }
}
