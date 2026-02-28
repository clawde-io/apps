/// Drift scanner — reads FEATURES.md and walks source files to detect drift.
///
/// Logic:
///  1. Parse FEATURES.md for lines containing ✅ with a feature name.
///  2. For each ✅ feature, derive candidate source identifiers (snake_case, kebab-case).
///  3. Walk source tree (*.rs, *.dart, *.ts, *.tsx) looking for the identifier.
///  4. If not found → emit a DriftItem with severity based on feature category.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Drift severity level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DriftSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl DriftSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            DriftSeverity::Critical => "critical",
            DriftSeverity::High => "high",
            DriftSeverity::Medium => "medium",
            DriftSeverity::Low => "low",
        }
    }
}

/// A detected drift item — a feature in spec with no source match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftItem {
    pub id: String,
    pub feature: String,
    pub severity: DriftSeverity,
    /// "missing_source" | "missing_handler" | "doc_only"
    pub kind: String,
    pub message: String,
    pub location: Option<String>,
    pub project_path: String,
}

/// Scan `project_path` for drift between FEATURES.md spec and source files.
///
/// Returns a list of drift items for features marked ✅ in FEATURES.md with no
/// corresponding source implementation found in the project's source tree.
pub async fn scan(project_path: &Path) -> Result<Vec<DriftItem>> {
    let project_path = project_path.to_path_buf();

    tokio::task::spawn_blocking(move || scan_sync(&project_path))
        .await
        .map_err(|e| anyhow::anyhow!("drift scan panicked: {e}"))?
}

fn scan_sync(project_path: &Path) -> Result<Vec<DriftItem>> {
    let mut items = Vec::new();

    // ── 1. Find FEATURES.md ──────────────────────────────────────────────────
    let features_path = find_features_md(project_path);
    let Some(features_path) = features_path else {
        // No FEATURES.md found — no drift to report
        return Ok(items);
    };

    let features_content = std::fs::read_to_string(&features_path).unwrap_or_default();

    // ── 2. Collect source file contents for identifier matching ──────────────
    let source_root = project_path;
    let source_texts = collect_source_texts(source_root);

    // ── 3. Parse ✅ features from FEATURES.md ─────────────────────────────────
    let done_features = parse_done_features(&features_content);

    // ── 4. For each done feature, check if source identifier exists ───────────
    for feature in done_features {
        let candidates = derive_identifiers(&feature);
        let found = candidates
            .iter()
            .any(|id| source_texts.iter().any(|src| src.contains(id.as_str())));

        if !found {
            let severity = infer_severity(&feature);
            let id = uuid::Uuid::new_v4().to_string();
            items.push(DriftItem {
                id,
                feature: feature.clone(),
                severity,
                kind: "missing_source".to_string(),
                message: format!(
                    "Feature '{feature}' is marked ✅ in FEATURES.md but no source \
                     identifier was found in the project."
                ),
                location: None,
                project_path: project_path.to_string_lossy().to_string(),
            });
        }
    }

    Ok(items)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Search for FEATURES.md in project_path and common sub-directories.
fn find_features_md(project_path: &Path) -> Option<PathBuf> {
    // Common locations: repo root, .claude/docs/, docs/
    let candidates = [
        project_path.join("FEATURES.md"),
        project_path.join(".claude/docs/FEATURES.md"),
        project_path.join("docs/FEATURES.md"),
        project_path.join(".docs/FEATURES.md"),
    ];
    candidates.into_iter().find(|c| c.exists())
}

/// Collect all source file contents (*.rs, *.dart, *.ts, *.tsx) up to depth 10.
fn collect_source_texts(root: &Path) -> Vec<String> {
    let mut texts = Vec::new();
    collect_recursive(root, &mut texts, 0, 10);
    texts
}

fn collect_recursive(dir: &Path, out: &mut Vec<String>, depth: usize, max_depth: usize) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Skip hidden directories, node_modules, target, .git
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }
        }
        if path.is_dir() {
            collect_recursive(&path, out, depth + 1, max_depth);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if matches!(ext, "rs" | "dart" | "ts" | "tsx" | "js" | "jsx") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    out.push(content);
                }
            }
        }
    }
}

/// Parse FEATURES.md lines that contain a ✅ status marker.
/// Extracts the feature name from the line (content after status emoji).
fn parse_done_features(content: &str) -> Vec<String> {
    let mut features = Vec::new();
    for line in content.lines() {
        // Match lines with ✅ — common formats:
        //   | ✅ | Feature Name | ... |
        //   - ✅ Feature Name
        //   * ✅ Feature Name
        if !line.contains('✅') {
            continue;
        }
        // Extract name: find text after ✅ up to | or end-of-meaningful-content
        let after = line.split('✅').nth(1).unwrap_or("").trim();

        // Strip leading/trailing table cell markers
        let name = after
            .trim_start_matches('|')
            .trim_end_matches('|')
            .split('|')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();

        // Strip inline markdown: remove **, *, `, [, ]
        let name = name.replace("**", "").replace(['*', '`', '[', ']'], "");

        let name = name.trim().to_string();
        if !name.is_empty() && name.len() > 2 {
            features.push(name);
        }
    }
    features
}

/// Derive candidate source identifiers for a feature name.
/// e.g. "Session Manager" → ["session_manager", "SessionManager", "session-manager"]
fn derive_identifiers(feature: &str) -> Vec<String> {
    let words: Vec<&str> = feature
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();

    let snake = words
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("_");

    let camel = words
        .iter()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => {
                    f.to_uppercase().collect::<String>() + c.as_str().to_lowercase().as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join("");

    let kebab = words
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("-");

    // Use the first significant word (≥4 chars) as a fallback
    let first_word = words
        .iter()
        .find(|w| w.len() >= 4)
        .map(|w| w.to_lowercase())
        .unwrap_or_default();

    let mut candidates = vec![snake, camel, kebab];
    if !first_word.is_empty() {
        candidates.push(first_word);
    }
    // Deduplicate
    candidates.sort();
    candidates.dedup();
    candidates
}

/// Infer drift severity from feature name keywords.
fn infer_severity(feature: &str) -> DriftSeverity {
    let lower = feature.to_lowercase();
    if lower.contains("auth")
        || lower.contains("security")
        || lower.contains("session")
        || lower.contains("license")
    {
        return DriftSeverity::High;
    }
    if lower.contains("rpc")
        || lower.contains("api")
        || lower.contains("handler")
        || lower.contains("storage")
        || lower.contains("database")
    {
        return DriftSeverity::Medium;
    }
    DriftSeverity::Low
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_identifiers() {
        let ids = derive_identifiers("Session Manager");
        assert!(ids.contains(&"session_manager".to_string()));
        assert!(ids.contains(&"SessionManager".to_string()));
    }

    #[test]
    fn test_parse_done_features() {
        let content = r#"
## Core Features

| Status | Feature | Notes |
|--------|---------|-------|
| ✅ | Session Manager | core RPC |
| ✅ | **Token Tracker** | phase 61 |
| 🔲 | Cloud Sync | future |
| - ✅ Live Diff View
"#;
        let features = parse_done_features(content);
        assert!(features.contains(&"Session Manager".to_string()));
        assert!(features.contains(&"Token Tracker".to_string()));
        assert!(!features.iter().any(|f| f.contains("Cloud Sync")));
    }

    #[test]
    fn test_infer_severity() {
        assert_eq!(infer_severity("Session Auth"), DriftSeverity::High);
        assert_eq!(infer_severity("RPC Handler"), DriftSeverity::Medium);
        assert_eq!(infer_severity("UI Color Theme"), DriftSeverity::Low);
    }
}
