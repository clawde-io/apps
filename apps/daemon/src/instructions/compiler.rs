// instructions/compiler.rs — Instruction graph compiler (Sprint ZZ IG.T02/T03)
//
// RPC: instructions.compile(target, project_path)  → CompileResult
// RPC: instructions.explain(path)                  → ExplainResult
// RPC: instructions.budgetReport(project_path)     → BudgetReport

use crate::storage::Storage;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Target format for compiled instructions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileTarget {
    Claude, // CLAUDE.md + .claude/rules/*.md
    Codex,  // AGENTS.md
    All,    // Both
}

impl CompileTarget {
    pub fn parse(s: &str) -> Self {
        match s {
            "codex" => Self::Codex,
            "all" => Self::All,
            _ => Self::Claude,
        }
    }

    pub fn output_filename(&self) -> &str {
        match self {
            Self::Claude => "CLAUDE.md",
            Self::Codex => "AGENTS.md",
            Self::All => "CLAUDE.md", // "all" writes CLAUDE.md first
        }
    }

    /// Max bytes for this target (8KB for claude, 64KB for codex)
    pub fn budget_bytes(&self) -> usize {
        match self {
            Self::Codex => 65536,
            _ => 8192,
        }
    }
}

#[derive(Debug)]
pub struct CompiledOutput {
    pub target: CompileTarget,
    pub content: String,
    pub instruction_hash: String,
    pub bytes_used: usize,
    pub budget_bytes: usize,
    pub node_count: usize,
    pub over_budget: bool,
    pub near_budget: bool, // > 80%
}

#[derive(Debug)]
pub struct NodeSource {
    pub id: String,
    pub scope: String,
    pub owner: Option<String>,
    pub priority: i64,
    pub preview: String, // first 80 chars
}

#[derive(Debug)]
pub struct ExplainResult {
    pub path: String,
    pub nodes: Vec<NodeSource>,
    pub merged_content: String,
    pub bytes_used: usize,
    pub budget_bytes: usize,
    pub conflicts: Vec<String>,
}

pub struct InstructionCompiler<'a> {
    storage: &'a Storage,
}

impl<'a> InstructionCompiler<'a> {
    pub fn new(storage: &'a Storage) -> Self {
        Self { storage }
    }

    /// Compile instruction nodes for a given target + project path.
    pub async fn compile(
        &self,
        target: CompileTarget,
        project_path: &str,
    ) -> Result<CompiledOutput> {
        let nodes = self.load_nodes_for_path(project_path).await?;
        let content = self.merge_nodes(&nodes, &target);
        let bytes_used = content.len();
        let budget_bytes = target.budget_bytes();
        let instruction_hash = format!("{:x}", Sha256::digest(content.as_bytes()));

        Ok(CompiledOutput {
            target,
            instruction_hash,
            bytes_used,
            budget_bytes,
            node_count: nodes.len(),
            over_budget: bytes_used > budget_bytes,
            near_budget: bytes_used > (budget_bytes * 80 / 100),
            content,
        })
    }

    /// Explain effective instructions for a given directory path.
    pub async fn explain(&self, path: &str) -> Result<ExplainResult> {
        let nodes = self.load_nodes_for_path(path).await?;
        let claude_target = CompileTarget::Claude;
        let merged_content = self.merge_nodes(&nodes, &claude_target);
        let bytes_used = merged_content.len();
        let budget_bytes = claude_target.budget_bytes();

        let node_sources = nodes
            .iter()
            .map(|n| NodeSource {
                id: n.id.clone(),
                scope: n.scope.clone(),
                owner: n.owner.clone(),
                priority: n.priority,
                preview: n.content_md.chars().take(80).collect(),
            })
            .collect();

        // Simple conflict detection — look for obvious contradictions
        let conflicts = self.detect_conflicts(&nodes);

        Ok(ExplainResult {
            path: path.to_string(),
            nodes: node_sources,
            merged_content,
            bytes_used,
            budget_bytes,
            conflicts,
        })
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    async fn load_nodes_for_path(&self, path: &str) -> Result<Vec<InstructionNode>> {
        let rows = sqlx::query_as::<_, InstructionNode>(
            "SELECT id, scope, scope_path, priority, owner, mode_overlays, content_md
             FROM instruction_nodes
             WHERE effective_date IS NULL OR effective_date <= date('now')
             ORDER BY
               CASE scope
                 WHEN 'path'    THEN 1
                 WHEN 'app'     THEN 2
                 WHEN 'project' THEN 3
                 WHEN 'org'     THEN 4
                 WHEN 'global'  THEN 5
                 ELSE 6
               END,
               priority ASC",
        )
        .fetch_all(self.storage.pool())
        .await
        .context("load instruction nodes")?;

        // Filter path-scoped nodes to those whose scope_path is a prefix of the requested path
        let filtered: Vec<InstructionNode> = rows
            .into_iter()
            .filter(|n| {
                if n.scope == "path" {
                    if let Some(ref sp) = n.scope_path {
                        return Path::new(path).starts_with(Path::new(sp));
                    }
                    return false;
                }
                true
            })
            .collect();

        Ok(filtered)
    }

    fn merge_nodes(&self, nodes: &[InstructionNode], target: &CompileTarget) -> String {
        let header = match target {
            CompileTarget::Claude => "# Compiled Instructions (ClawDE instruction graph)\n\n",
            CompileTarget::Codex => {
                "# AGENTS.md — Compiled Instructions (ClawDE instruction graph)\n\n"
            }
            CompileTarget::All => "# Compiled Instructions (ClawDE instruction graph)\n\n",
        };

        let mut parts = vec![header.to_string()];
        for node in nodes {
            parts.push(format!(
                "<!-- scope:{} priority:{} -->\n{}\n\n",
                node.scope,
                node.priority,
                node.content_md.trim()
            ));
        }
        parts.concat()
    }

    fn detect_conflicts(&self, nodes: &[InstructionNode]) -> Vec<String> {
        let mut conflicts = Vec::new();

        // Check for package manager contradictions
        let has_npm = nodes
            .iter()
            .any(|n| n.content_md.contains("use npm") || n.content_md.contains("npm install"));
        let has_pnpm = nodes
            .iter()
            .any(|n| n.content_md.contains("use pnpm") || n.content_md.contains("pnpm install"));
        let has_yarn = nodes
            .iter()
            .any(|n| n.content_md.contains("use yarn") || n.content_md.contains("yarn install"));
        let pm_count = [has_npm, has_pnpm, has_yarn].iter().filter(|&&b| b).count();
        if pm_count > 1 {
            conflicts
                .push("Conflicting package manager rules detected (npm/pnpm/yarn)".to_string());
        }

        conflicts
    }
}

#[derive(Debug, sqlx::FromRow)]
struct InstructionNode {
    pub id: String,
    pub scope: String,
    pub scope_path: Option<String>,
    pub priority: i64,
    pub owner: Option<String>,
    #[allow(dead_code)]
    pub mode_overlays: Option<String>,
    pub content_md: String,
}
