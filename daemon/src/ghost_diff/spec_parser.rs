//! Sprint CC GD.2 — Spec file parser.
//!
//! Spec files live in `.claw/specs/{component}.md`.
//! Each file is plain markdown with "Expected behavior" sections.
//! The parser extracts the expected behavior bullet points for comparison.

use anyhow::{Context, Result};
use std::path::Path;
use tracing::warn;

/// A parsed spec file.
#[derive(Debug, Clone)]
pub struct SpecFile {
    /// The filename stem (e.g. `"session"` from `session.md`).
    pub name: String,
    /// List of expected-behavior descriptions extracted from the spec.
    pub expected_behaviors: Vec<String>,
}

/// Load all spec files from a directory.
pub fn load_specs(specs_dir: &Path) -> Result<Vec<SpecFile>> {
    if !specs_dir.exists() {
        return Ok(vec![]);
    }

    let mut specs = Vec::new();
    let entries = std::fs::read_dir(specs_dir)
        .with_context(|| format!("read specs dir {}", specs_dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            match parse_spec_file(&path) {
                Ok(spec) => specs.push(spec),
                Err(e) => warn!(path = %path.display(), "failed to parse spec: {e}"),
            }
        }
    }

    Ok(specs)
}

/// Parse a single spec file. Extracts "Expected behavior" sections.
fn parse_spec_file(path: &Path) -> Result<SpecFile> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let content =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;

    let behaviors = extract_expected_behaviors(&content);

    Ok(SpecFile {
        name,
        expected_behaviors: behaviors,
    })
}

/// Extract bullet points from "Expected behavior" sections.
///
/// Recognizes headers like:
/// - `## Expected behavior`
/// - `### Expected behaviors`
/// - `## Specification`
fn extract_expected_behaviors(content: &str) -> Vec<String> {
    let mut in_section = false;
    let mut behaviors = Vec::new();

    for line in content.lines() {
        let lower = line.to_lowercase();

        // Detect section headers.
        if line.starts_with('#') {
            in_section = lower.contains("expected") || lower.contains("specification")
                || lower.contains("behavior") || lower.contains("requirements");
            continue;
        }

        if in_section {
            // Stop at next heading.
            if line.starts_with('#') {
                in_section = false;
                continue;
            }
            // Collect bullet points and non-empty lines.
            let trimmed = line.trim_start_matches(['-', '*', ' ']);
            if !trimmed.is_empty() && trimmed.len() > 5 {
                behaviors.push(trimmed.to_string());
            }
        }
    }

    // If no section found, use the whole document as behaviors.
    if behaviors.is_empty() {
        behaviors = content
            .lines()
            .filter(|l| !l.starts_with('#') && l.trim().len() > 10)
            .map(|l| l.trim().to_string())
            .take(20)
            .collect();
    }

    behaviors
}
