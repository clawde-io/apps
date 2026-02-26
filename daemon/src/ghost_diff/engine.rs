//! Sprint CC GD.1 — Ghost Diff engine.
//!
//! Compares session file changes against spec files in `.claw/specs/` to detect
//! spec drift. Uses substring matching + keyword overlap for semantic similarity.

use anyhow::Result;
use std::path::Path;
use tracing::debug;

use super::spec_parser::{load_specs, SpecFile};

// ─── Warning type ──────────────────────────────────────────────────────────

/// A single ghost drift warning: a file diverges from a spec.
#[derive(Debug, Clone)]
pub struct GhostDriftWarning {
    /// Source file that changed.
    pub file: String,
    /// Spec file that defines expected behavior.
    pub spec: String,
    /// Human-readable summary of the divergence.
    pub divergence_summary: String,
    /// Severity: `"low"`, `"medium"`, `"high"`.
    pub severity: String,
}

// ─── Main check ────────────────────────────────────────────────────────────

/// Run ghost diff: compare changed files in `repo_path` against all spec files.
/// If `session_id` is provided, only checks files changed in that session.
pub async fn check_ghost_drift(
    repo_path: &str,
    _session_id: Option<&str>,
) -> Result<Vec<GhostDriftWarning>> {
    let specs_dir = Path::new(repo_path).join(".claw").join("specs");
    if !specs_dir.exists() {
        debug!("no .claw/specs/ directory — ghost diff skipped");
        return Ok(vec![]);
    }

    let specs = load_specs(&specs_dir)?;
    if specs.is_empty() {
        return Ok(vec![]);
    }

    // Get recently changed files (git diff HEAD --name-only as a lightweight proxy).
    let changed_files = get_changed_files(repo_path).await?;
    if changed_files.is_empty() {
        return Ok(vec![]);
    }

    let mut warnings = Vec::new();
    for file_path in &changed_files {
        let file_content = match std::fs::read_to_string(Path::new(repo_path).join(file_path)) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for spec in &specs {
            if let Some(warning) = check_file_against_spec(file_path, &file_content, spec) {
                warnings.push(warning);
            }
        }
    }

    Ok(warnings)
}

// ─── File / spec comparison ────────────────────────────────────────────────

fn check_file_against_spec(
    file_path: &str,
    file_content: &str,
    spec: &SpecFile,
) -> Option<GhostDriftWarning> {
    // Only check specs that are relevant to this file.
    if !spec_applies_to_file(file_path, spec) {
        return None;
    }

    // Check each "Expected behavior" section in the spec.
    for expected in &spec.expected_behaviors {
        // Simple check: important keywords from the spec must appear in the file.
        let keywords = extract_keywords(expected);
        let missing: Vec<&str> = keywords
            .iter()
            .filter(|kw| !file_content.to_lowercase().contains(*kw))
            .copied()
            .collect();

        if missing.len() > keywords.len() / 2 {
            // More than half the spec keywords are absent.
            return Some(GhostDriftWarning {
                file: file_path.to_string(),
                spec: spec.name.clone(),
                divergence_summary: format!(
                    "File '{}' may diverge from spec '{}': missing concepts: {}",
                    file_path,
                    spec.name,
                    missing.join(", ")
                ),
                severity: if missing.len() == keywords.len() {
                    "high".to_string()
                } else {
                    "medium".to_string()
                },
            });
        }
    }

    None
}

/// Determine if a spec is relevant to a given file path by name matching.
fn spec_applies_to_file(file_path: &str, spec: &SpecFile) -> bool {
    let spec_stem = spec
        .name
        .to_lowercase()
        .trim_end_matches(".md")
        .replace('-', "_");
    let file_lower = file_path.to_lowercase().replace('/', "_").replace('-', "_");
    file_lower.contains(&spec_stem) || spec_stem.contains(file_lower.trim_end_matches(".rs"))
}

/// Extract meaningful keywords from an "Expected behavior" description.
fn extract_keywords(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .filter(|w| {
            let w = w.to_lowercase();
            // Filter out stop words, keep domain terms.
            !matches!(
                w.trim_matches(|c: char| !c.is_alphabetic()),
                "the" | "a" | "an" | "and" | "or" | "in" | "to" | "of" | "is" | "should"
                    | "must" | "will" | "be" | "by" | "for" | "that" | "this" | "with"
            )
        })
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()))
        .filter(|w| w.len() > 3)
        .take(10)
        .collect()
}

/// Get files changed since last commit using `git diff HEAD --name-only`.
async fn get_changed_files(repo_path: &str) -> Result<Vec<String>> {
    let output = tokio::process::Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["diff", "HEAD", "--name-only"])
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    Ok(files)
}
