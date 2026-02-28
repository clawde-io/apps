// SPDX-License-Identifier: MIT
// Sprint N — Topology data model (MR.T01).

use serde::{Deserialize, Serialize};

// ─── DepType ─────────────────────────────────────────────────────────────────

/// The semantic relationship between two repos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepType {
    /// The downstream repo is a Cargo/pub workspace member or direct library
    /// dependency of the upstream repo.
    BuildsOn,
    /// The downstream repo calls the upstream repo's API at runtime
    /// (imports paths contain the upstream repo name).
    UsesApi,
    /// Both repos share a protocol definition package (e.g. clawd_proto ↔
    /// @clawde/proto carry the same types in different languages).
    SharesTypes,
    /// The downstream repo is deployed together with the upstream repo as part
    /// of the same release (e.g. daemon + desktop go out in the same binary).
    DeploysWith,
}

impl DepType {
    /// Canonical string used in the database `dep_type` column.
    pub fn as_str(&self) -> &'static str {
        match self {
            DepType::BuildsOn => "builds_on",
            DepType::UsesApi => "uses_api",
            DepType::SharesTypes => "shares_types",
            DepType::DeploysWith => "deploys_with",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> DepType {
        match s {
            "builds_on" => DepType::BuildsOn,
            "shares_types" => DepType::SharesTypes,
            "deploys_with" => DepType::DeploysWith,
            _ => DepType::UsesApi,
        }
    }
}

// ─── RepoNode ─────────────────────────────────────────────────────────────────

/// A registered repository that appears as a node in the topology graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoNode {
    /// Absolute filesystem path (e.g. `/Users/alice/Sites/myapp`).
    pub path: String,
    /// Human-readable name derived from the path's last component.
    pub name: String,
    /// Aggregate health score 0–100 (derived from drift score when available,
    /// or 100 when no profile has been scanned yet).
    pub health_score: u8,
    /// Detected tech stack labels (e.g. `["rust", "flutter"]`).
    pub stack: Vec<String>,
}

impl RepoNode {
    /// Construct a minimal node from just an absolute path.
    pub fn from_path(path: &str) -> RepoNode {
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path)
            .to_string();
        RepoNode {
            path: path.to_string(),
            name,
            health_score: 100,
            stack: Vec::new(),
        }
    }
}

// ─── Dependency ───────────────────────────────────────────────────────────────

/// A directed dependency edge from one repo to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub id: String,
    pub from_repo: String,
    pub to_repo: String,
    pub dep_type: DepType,
    /// Heuristic confidence 0.0–1.0.  Manually-declared edges always use 1.0.
    pub confidence: f64,
    /// True when discovered automatically by the detector.
    pub auto_detected: bool,
    pub created_at: String,
}

// ─── TopologyGraph ────────────────────────────────────────────────────────────

/// The full multi-repo dependency graph.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TopologyGraph {
    pub nodes: Vec<RepoNode>,
    pub edges: Vec<Dependency>,
}

// ─── CrossValidationResult ────────────────────────────────────────────────────

/// Result of a single cross-repo type/contract check (MR.T11).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossValidationResult {
    pub source_repo: String,
    pub target_repo: String,
    /// Short identifier for the check (e.g. `"proto_types_match"`).
    pub check: String,
    pub passed: bool,
    pub detail: String,
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_type_as_str_roundtrip() {
        let cases = [
            (DepType::BuildsOn, "builds_on"),
            (DepType::UsesApi, "uses_api"),
            (DepType::SharesTypes, "shares_types"),
            (DepType::DeploysWith, "deploys_with"),
        ];
        for (variant, expected) in cases {
            assert_eq!(variant.as_str(), expected);
        }
    }

    #[test]
    fn dep_type_from_str_known_values() {
        assert_eq!(DepType::from_str("builds_on"), DepType::BuildsOn);
        assert_eq!(DepType::from_str("shares_types"), DepType::SharesTypes);
        assert_eq!(DepType::from_str("deploys_with"), DepType::DeploysWith);
    }

    #[test]
    fn dep_type_from_str_unknown_falls_back_to_uses_api() {
        assert_eq!(DepType::from_str(""), DepType::UsesApi);
        assert_eq!(DepType::from_str("unknown"), DepType::UsesApi);
        assert_eq!(DepType::from_str("uses_api"), DepType::UsesApi);
    }

    #[test]
    fn repo_node_from_path_extracts_name() {
        let node = RepoNode::from_path("/Users/alice/Sites/myapp");
        assert_eq!(node.name, "myapp");
        assert_eq!(node.path, "/Users/alice/Sites/myapp");
        assert_eq!(node.health_score, 100);
        assert!(node.stack.is_empty());
    }

    #[test]
    fn repo_node_from_path_root_does_not_panic() {
        let node = RepoNode::from_path("/");
        // file_name() on "/" returns None — falls back to path itself
        assert!(!node.name.is_empty());
    }

    #[test]
    fn topology_graph_default_is_empty() {
        let graph = TopologyGraph::default();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn cross_validation_result_fields() {
        let result = CrossValidationResult {
            source_repo: "apps".to_string(),
            target_repo: "web".to_string(),
            check: "proto_types_match".to_string(),
            passed: true,
            detail: "All 17 RPC types match".to_string(),
        };
        assert!(result.passed);
        assert_eq!(result.check, "proto_types_match");
    }

    #[test]
    fn dependency_edge_fields() {
        let dep = Dependency {
            id: "dep-1".to_string(),
            from_repo: "desktop".to_string(),
            to_repo: "daemon".to_string(),
            dep_type: DepType::BuildsOn,
            confidence: 1.0,
            auto_detected: false,
            created_at: "2026-02-25T00:00:00Z".to_string(),
        };
        assert_eq!(dep.dep_type, DepType::BuildsOn);
        assert!((dep.confidence - 1.0).abs() < f64::EPSILON);
        assert!(!dep.auto_detected);
    }
}
