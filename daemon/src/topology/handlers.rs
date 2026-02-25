// SPDX-License-Identifier: MIT
// Sprint N — Topology JSON-RPC 2.0 handlers (MR.T03, MR.T04, MR.T11, MR.T12).
//
// Registered methods (wired in ipc/mod.rs):
//   topology.get              — return the full dependency graph
//   topology.validate         — check for cycles and missing repos
//   topology.addDependency    — manually declare a dependency edge
//   topology.removeDependency — remove an edge by id
//   topology.crossValidate    — run cross-repo type/contract validators

use crate::topology::model::{CrossValidationResult, DepType};
use crate::topology::storage::TopologyStorage;
use crate::AppContext;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

// ─── Param structs ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddDependencyParams {
    from_repo: String,
    to_repo: String,
    #[serde(default = "default_dep_type")]
    dep_type: String,
    #[serde(default = "default_confidence")]
    confidence: f64,
}

fn default_dep_type() -> String {
    "uses_api".to_string()
}

fn default_confidence() -> f64 {
    1.0
}

#[derive(Deserialize)]
struct RemoveDependencyParams {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CrossValidateParams {
    /// Limit cross-validation to this repo's outbound edges.  If omitted,
    /// all edges in the graph are validated.
    repo_path: Option<String>,
}

// ─── topology.get ──────────────────────────────────────────────────────────────

/// `topology.get` — return the full topology graph (nodes + edges).
pub async fn topology_get(_params: Value, ctx: &AppContext) -> Result<Value> {
    let storage = TopologyStorage::new(ctx.storage.pool());
    let graph = storage.get_topology().await?;
    Ok(serde_json::to_value(&graph)?)
}

// ─── topology.validate ────────────────────────────────────────────────────────

/// `topology.validate` — detect circular dependencies and report repos that are
/// referenced in edges but not registered with the daemon.
pub async fn topology_validate(_params: Value, ctx: &AppContext) -> Result<Value> {
    let storage = TopologyStorage::new(ctx.storage.pool());
    let graph = storage.get_topology().await?;
    let cycles = storage.find_cycles().await?;

    // Collect all repo paths referenced by edges.
    let mut edge_repos: std::collections::HashSet<String> = std::collections::HashSet::new();
    for e in &graph.edges {
        edge_repos.insert(e.from_repo.clone());
        edge_repos.insert(e.to_repo.clone());
    }

    // Find repos mentioned in edges that do not exist on disk.
    let mut missing: Vec<String> = edge_repos
        .iter()
        .filter(|p| !Path::new(p.as_str()).exists())
        .cloned()
        .collect();
    missing.sort();

    let valid = cycles.is_empty() && missing.is_empty();
    Ok(json!({
        "valid":   valid,
        "cycles":  cycles,
        "missing": missing,
    }))
}

// ─── topology.addDependency ───────────────────────────────────────────────────

/// `topology.addDependency` — manually declare a dependency edge between two repos.
pub async fn topology_add_dependency(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: AddDependencyParams = serde_json::from_value(params)?;

    validate_path("fromRepo", &p.from_repo)?;
    validate_path("toRepo", &p.to_repo)?;

    if p.from_repo == p.to_repo {
        bail!("fromRepo and toRepo must be different");
    }

    let dep_type = DepType::from_str(&p.dep_type);
    let storage = TopologyStorage::new(ctx.storage.pool());
    let dep = storage
        .add_dependency(&p.from_repo, &p.to_repo, &dep_type, p.confidence, false)
        .await?;

    Ok(serde_json::to_value(&dep)?)
}

// ─── topology.removeDependency ────────────────────────────────────────────────

/// `topology.removeDependency` — remove a dependency edge by its id.
pub async fn topology_remove_dependency(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RemoveDependencyParams = serde_json::from_value(params)?;
    if p.id.is_empty() {
        bail!("id is required");
    }
    let storage = TopologyStorage::new(ctx.storage.pool());
    storage.remove_dependency(&p.id).await?;
    Ok(json!({ "removed": true, "id": p.id }))
}

// ─── topology.crossValidate ──────────────────────────────────────────────────

/// `topology.crossValidate` — run cross-repo validators over dependency edges.
///
/// Currently implements one built-in check: verify that `clawd_proto` (Dart)
/// and `@clawde/proto` (TypeScript) declare at least one common type name.
///
/// Additional validators can be added here as Sprint N matures.
pub async fn topology_cross_validate(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: CrossValidateParams =
        serde_json::from_value(params).unwrap_or(CrossValidateParams { repo_path: None });
    let storage = TopologyStorage::new(ctx.storage.pool());
    let graph = storage.get_topology().await?;

    let edges_to_check: Vec<_> = if let Some(ref rp) = p.repo_path {
        graph.edges.iter().filter(|e| &e.from_repo == rp).collect()
    } else {
        graph.edges.iter().collect()
    };

    let mut results: Vec<CrossValidationResult> = Vec::new();

    for edge in edges_to_check {
        if edge.dep_type == crate::topology::model::DepType::SharesTypes {
            let result = validate_shared_types(&edge.from_repo, &edge.to_repo);
            results.push(result);
        }
    }

    Ok(serde_json::to_value(&results)?)
}

// ─── Cross-validation helper ──────────────────────────────────────────────────

/// Compare exported type names between two repos that share types.
///
/// Checks whether both repos export a non-empty set of identifiers, and
/// whether those sets overlap (heuristic: count matching identifiers).
fn validate_shared_types(from_repo: &str, to_repo: &str) -> CrossValidationResult {
    let from_types = collect_exported_type_names(Path::new(from_repo));
    let to_types = collect_exported_type_names(Path::new(to_repo));

    let intersection_count = from_types
        .iter()
        .filter(|name| to_types.contains(name.as_str()))
        .count();

    let passed = !from_types.is_empty() && !to_types.is_empty() && intersection_count > 0;
    let detail = if passed {
        format!("{intersection_count} shared type name(s) found")
    } else if from_types.is_empty() {
        format!("no exported types found in {from_repo}")
    } else if to_types.is_empty() {
        format!("no exported types found in {to_repo}")
    } else {
        "no overlapping type names found — possible type mismatch".to_string()
    };

    CrossValidationResult {
        source_repo: from_repo.to_string(),
        target_repo: to_repo.to_string(),
        check: "shared_type_names".to_string(),
        passed,
        detail,
    }
}

/// Collect a set of exported type / class / struct names from a repo directory.
///
/// Scans Dart files for `class Foo` / `enum Foo` / `typedef Foo` and Rust files
/// for `pub struct Foo` / `pub enum Foo`.  Returns empty set on I/O errors.
fn collect_exported_type_names(dir: &Path) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    collect_type_names_recursive(dir, &mut names, 0);
    names
}

fn collect_type_names_recursive(
    dir: &Path,
    names: &mut std::collections::HashSet<String>,
    depth: u32,
) {
    if depth > 6 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if file_name.starts_with('.')
            || matches!(
                file_name,
                "node_modules" | "target" | "build" | ".dart_tool"
            )
        {
            continue;
        }

        if path.is_dir() {
            collect_type_names_recursive(&path, names, depth + 1);
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            match ext {
                "dart" => extract_dart_type_names(&content, names),
                "ts" | "tsx" => extract_ts_type_names(&content, names),
                "rs" => extract_rust_type_names(&content, names),
                _ => {}
            }
        }
    }
}

fn extract_dart_type_names(content: &str, names: &mut std::collections::HashSet<String>) {
    for line in content.lines() {
        let line = line.trim();
        for prefix in &["class ", "enum ", "typedef ", "abstract class ", "mixin "] {
            if let Some(rest) = line.strip_prefix(prefix) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    names.insert(name);
                }
                break;
            }
        }
    }
}

fn extract_ts_type_names(content: &str, names: &mut std::collections::HashSet<String>) {
    for line in content.lines() {
        let line = line.trim();
        for prefix in &[
            "export interface ",
            "export type ",
            "export class ",
            "export enum ",
        ] {
            if let Some(rest) = line.strip_prefix(prefix) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    names.insert(name);
                }
                break;
            }
        }
    }
}

fn extract_rust_type_names(content: &str, names: &mut std::collections::HashSet<String>) {
    for line in content.lines() {
        let line = line.trim();
        for prefix in &["pub struct ", "pub enum ", "pub type "] {
            if let Some(rest) = line.strip_prefix(prefix) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    names.insert(name);
                }
                break;
            }
        }
    }
}

// ─── Validation helper ────────────────────────────────────────────────────────

fn validate_path(field: &str, value: &str) -> Result<()> {
    if value.contains('\0') {
        bail!("invalid {field}: null byte");
    }
    if !Path::new(value).is_absolute() {
        bail!("invalid {field}: must be an absolute path");
    }
    Ok(())
}
