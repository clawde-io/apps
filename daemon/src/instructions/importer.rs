// instructions/importer.rs — Import CLAUDE.md @path directives into instruction graph (Sprint ZZ IG.T04)
//
// `clawd instructions import` scans a project's existing .claude/CLAUDE.md (and any @path-imported
// files) and seeds instruction_nodes from them. One-way import: existing files → graph only.

use crate::storage::Storage;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct InstructionImporter<'a> {
    storage: &'a Storage,
}

#[derive(Debug)]
pub struct ImportResult {
    pub files_scanned: usize,
    pub nodes_created: usize,
    pub nodes_skipped: usize,
}

impl<'a> InstructionImporter<'a> {
    pub fn new(storage: &'a Storage) -> Self {
        Self { storage }
    }

    /// Import a project's existing CLAUDE.md and .claude/rules/ into instruction_nodes.
    pub async fn import_project(&self, project_path: &str) -> Result<ImportResult> {
        let base = Path::new(project_path);
        let mut files_scanned = 0;
        let mut nodes_created = 0;
        let mut nodes_skipped = 0;

        // Primary: CLAUDE.md at project root
        let claude_md = base.join("CLAUDE.md");
        if claude_md.exists() {
            let content = fs::read_to_string(&claude_md)
                .await
                .context("read CLAUDE.md")?;
            files_scanned += 1;
            let created = self.upsert_node("project", None, 100, &content).await?;
            if created { nodes_created += 1; } else { nodes_skipped += 1; }
        }

        // .claude/CLAUDE.md
        let dot_claude_md = base.join(".claude/CLAUDE.md");
        if dot_claude_md.exists() {
            let content = fs::read_to_string(&dot_claude_md)
                .await
                .context("read .claude/CLAUDE.md")?;
            files_scanned += 1;
            let created = self.upsert_node("project", None, 90, &content).await?;
            if created { nodes_created += 1; } else { nodes_skipped += 1; }
        }

        // .claude/rules/ directory — path-scoped rules
        let rules_dir = base.join(".claude/rules");
        if rules_dir.exists() {
            let mut dir = fs::read_dir(&rules_dir).await?;
            while let Some(entry) = dir.next_entry().await? {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    let content = fs::read_to_string(&path)
                        .await
                        .context("read rules file")?;
                    files_scanned += 1;
                    // Derive scope_path from filename (e.g. "rust.md" → no path scope, "src-payments.md" → src/payments/)
                    let created = self.upsert_node("app", None, 80, &content).await?;
                    if created { nodes_created += 1; } else { nodes_skipped += 1; }
                }
            }
        }

        Ok(ImportResult { files_scanned, nodes_created, nodes_skipped })
    }

    async fn upsert_node(&self, scope: &str, scope_path: Option<&str>, priority: i64, content: &str) -> Result<bool> {
        // Skip very short content (probably empty)
        if content.trim().len() < 10 {
            return Ok(false);
        }

        let result = sqlx::query(
            "INSERT INTO instruction_nodes (scope, scope_path, priority, owner, content_md)
             VALUES (?, ?, ?, 'imported', ?)
             ON CONFLICT DO NOTHING",
        )
        .bind(scope)
        .bind(scope_path)
        .bind(priority)
        .bind(content)
        .execute(self.storage.pool())
        .await
        .context("upsert instruction node")?;

        Ok(result.rows_affected() > 0)
    }
}
