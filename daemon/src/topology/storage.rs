// SPDX-License-Identifier: MIT
// Sprint N — Topology SQLite storage (MR.T01, MR.T03).

use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::model::{DepType, Dependency, RepoNode, TopologyGraph};

// ─── Raw DB row ───────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct DepRow {
    id: String,
    from_repo: String,
    to_repo: String,
    dep_type: String,
    confidence: f64,
    auto_detected: i64,
    created_at: String,
}

impl From<DepRow> for Dependency {
    fn from(r: DepRow) -> Dependency {
        Dependency {
            id: r.id,
            from_repo: r.from_repo,
            to_repo: r.to_repo,
            dep_type: DepType::from_str(&r.dep_type),
            confidence: r.confidence,
            auto_detected: r.auto_detected != 0,
            created_at: r.created_at,
        }
    }
}

// ─── TopologyStorage ──────────────────────────────────────────────────────────

/// SQLite-backed storage for the repo dependency topology.
#[derive(Clone)]
pub struct TopologyStorage {
    pool: SqlitePool,
}

impl TopologyStorage {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ── Mutation ──────────────────────────────────────────────────────────────

    /// Upsert a dependency edge.  If an edge between the same (from, to) pair
    /// already exists it is updated in place (preserving the original id).
    pub async fn add_dependency(
        &self,
        from_repo: &str,
        to_repo: &str,
        dep_type: &DepType,
        confidence: f64,
        auto_detected: bool,
    ) -> Result<Dependency> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let dep_type_str = dep_type.as_str();
        let auto_int: i64 = if auto_detected { 1 } else { 0 };

        sqlx::query(
            "INSERT INTO repo_dependencies
                 (id, from_repo, to_repo, dep_type, confidence, auto_detected, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(from_repo, to_repo) DO UPDATE SET
                 dep_type      = excluded.dep_type,
                 confidence    = excluded.confidence,
                 auto_detected = excluded.auto_detected",
        )
        .bind(&id)
        .bind(from_repo)
        .bind(to_repo)
        .bind(dep_type_str)
        .bind(confidence)
        .bind(auto_int)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        // Re-read so that the returned row reflects the actual stored values
        // (in case the ON CONFLICT branch ran and preserved the old id).
        let row: DepRow =
            sqlx::query_as("SELECT * FROM repo_dependencies WHERE from_repo = ? AND to_repo = ?")
                .bind(from_repo)
                .bind(to_repo)
                .fetch_one(&self.pool)
                .await?;

        Ok(row.into())
    }

    /// Delete a dependency edge identified by `id`.
    pub async fn remove_dependency(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM repo_dependencies WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Return the full topology graph.
    ///
    /// Nodes are derived from the distinct repo paths referenced by the edges;
    /// orphan repos (registered but with no edges) are not included here — the
    /// caller is responsible for merging with the repo registry if needed.
    pub async fn get_topology(&self) -> Result<TopologyGraph> {
        let rows: Vec<DepRow> =
            sqlx::query_as("SELECT * FROM repo_dependencies ORDER BY created_at ASC")
                .fetch_all(&self.pool)
                .await?;

        let edges: Vec<Dependency> = rows.into_iter().map(Into::into).collect();

        // Collect unique repo paths and build minimal nodes.
        let mut paths: std::collections::HashSet<String> = std::collections::HashSet::new();
        for e in &edges {
            paths.insert(e.from_repo.clone());
            paths.insert(e.to_repo.clone());
        }
        let nodes: Vec<RepoNode> = paths.iter().map(|p| RepoNode::from_path(p)).collect();

        Ok(TopologyGraph { nodes, edges })
    }

    /// Return all dependency edges involving the given repo (as source or target).
    pub async fn list_dependencies(&self, repo_path: &str) -> Result<Vec<Dependency>> {
        let rows: Vec<DepRow> = sqlx::query_as(
            "SELECT * FROM repo_dependencies
             WHERE from_repo = ? OR to_repo = ?
             ORDER BY created_at ASC",
        )
        .bind(repo_path)
        .bind(repo_path)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Detect circular dependencies by following the edge graph depth-first.
    ///
    /// Returns a list of cycle paths (each represented as `"A -> B -> A"`).
    pub async fn find_cycles(&self) -> Result<Vec<String>> {
        let graph = self.get_topology().await?;
        let mut cycles: Vec<String> = Vec::new();

        // Build adjacency map.
        let mut adj: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for edge in &graph.edges {
            adj.entry(edge.from_repo.clone())
                .or_default()
                .push(edge.to_repo.clone());
        }

        // DFS for each starting node.
        let nodes: Vec<String> = graph.nodes.iter().map(|n| n.path.clone()).collect();

        for start in &nodes {
            let mut visited: Vec<String> = Vec::new();
            dfs_find_cycle(start, &adj, &mut visited, &mut cycles);
        }

        cycles.sort();
        cycles.dedup();
        Ok(cycles)
    }
}

// ─── DFS helper ───────────────────────────────────────────────────────────────

fn dfs_find_cycle(
    node: &str,
    adj: &std::collections::HashMap<String, Vec<String>>,
    path: &mut Vec<String>,
    cycles: &mut Vec<String>,
) {
    if let Some(pos) = path.iter().position(|p| p == node) {
        // Found a back-edge — record the cycle.
        let cycle_nodes = &path[pos..];
        let mut desc = cycle_nodes.join(" -> ");
        desc.push_str(" -> ");
        desc.push_str(node);
        cycles.push(desc);
        return;
    }
    path.push(node.to_string());
    if let Some(neighbors) = adj.get(node) {
        for next in neighbors {
            dfs_find_cycle(next, adj, path, cycles);
        }
    }
    path.pop();
}
