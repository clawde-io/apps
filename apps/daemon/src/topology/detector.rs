// SPDX-License-Identifier: MIT
// Sprint N — Dependency auto-detector (MR.T02).
//
// Heuristic scan of repo manifests and import paths to infer dependency edges.
// The results are suggestions (confidence < 1.0) that the user can confirm or
// remove via the manual topology.addDependency / topology.removeDependency RPCs.

use std::path::Path;

use super::model::{DepType, Dependency};
use chrono::Utc;
use uuid::Uuid;

/// Scan `repo_path` for references to any of the repos listed in `all_repos`
/// and return a list of inferred `Dependency` edges.
///
/// Three heuristics are applied in order:
///
/// 1. **Cargo.toml workspace members** — `members = [...]` lists sub-crates;
///    if those paths correspond to another registered repo, classify as
///    `builds_on` (confidence 0.9).
///
/// 2. **package.json workspaces** — `workspaces: [...]` arrays; same logic,
///    classified as `shares_types` (confidence 0.85).
///
/// 3. **Import path scan** — all `*.dart`, `*.ts`, `*.rs`, `*.py` files are
///    searched for string occurrences of the other repo's name; if found,
///    classified as `uses_api` (confidence 0.6).
pub fn auto_detect_dependencies(repo_path: &Path, all_repos: &[String]) -> Vec<Dependency> {
    let mut deps: Vec<Dependency> = Vec::new();
    let from = repo_path.to_string_lossy().to_string();

    for other in all_repos {
        // Skip self-references.
        if other == &from {
            continue;
        }

        let other_name = Path::new(other)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(other.as_str())
            .to_string();

        // ── Heuristic 1: Cargo.toml workspace members ───────────────────────
        let cargo_path = repo_path.join("Cargo.toml");
        if cargo_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&cargo_path) {
                if cargo_members_reference(&content, &other_name)
                    || content.contains(other.as_str())
                {
                    deps.push(make_dep(&from, other, DepType::BuildsOn, 0.9, true));
                    continue;
                }
            }
        }

        // ── Heuristic 2: package.json workspaces ────────────────────────────
        let pkg_json_path = repo_path.join("package.json");
        if pkg_json_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&pkg_json_path) {
                if content.contains(&other_name) || content.contains(other.as_str()) {
                    deps.push(make_dep(&from, other, DepType::SharesTypes, 0.85, true));
                    continue;
                }
            }
        }

        // ── Heuristic 3: import path scan ───────────────────────────────────
        if scan_imports_for_name(repo_path, &other_name) {
            deps.push(make_dep(&from, other, DepType::UsesApi, 0.6, true));
        }
    }

    deps
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn make_dep(
    from: &str,
    to: &str,
    dep_type: DepType,
    confidence: f64,
    auto_detected: bool,
) -> Dependency {
    Dependency {
        id: Uuid::new_v4().to_string(),
        from_repo: from.to_string(),
        to_repo: to.to_string(),
        dep_type,
        confidence,
        auto_detected,
        created_at: Utc::now().to_rfc3339(),
    }
}

/// Check if a Cargo.toml workspace `members` list references `name`.
fn cargo_members_reference(cargo_content: &str, name: &str) -> bool {
    // Quick text search — does not require a full TOML parser.
    // Looks for the name inside a `[workspace]` members list.
    let in_workspace_section = cargo_content.contains("[workspace]");
    if !in_workspace_section {
        return false;
    }
    cargo_content.contains(name)
}

/// Walk source files in `repo_path` looking for any file that imports / uses
/// a string matching `other_name`.  Searches .dart, .ts, .tsx, .rs, .py files.
fn scan_imports_for_name(repo_path: &Path, other_name: &str) -> bool {
    let extensions = ["dart", "ts", "tsx", "rs", "py"];
    scan_dir(repo_path, &extensions, other_name, 0)
}

fn scan_dir(dir: &Path, exts: &[&str], needle: &str, depth: u32) -> bool {
    if depth > 6 {
        return false;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden directories and common non-source dirs.
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name.starts_with('.')
            || matches!(name, "node_modules" | "target" | "build" | ".dart_tool")
        {
            continue;
        }

        if path.is_dir() {
            if scan_dir(&path, exts, needle, depth + 1) {
                return true;
            }
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            if exts.contains(&ext) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.contains(needle) {
                        return true;
                    }
                }
            }
        }
    }
    false
}
