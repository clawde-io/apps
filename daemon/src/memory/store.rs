// memory/store.rs — Persistent AI memory store.
//
// Sprint OO ME.1: MemoryStore + MemoryEntry — global + project scopes.
//
// Memory entries are stored in SQLite (memory table). Each entry has:
//   - scope: "global" (all repos) or SHA-256 of the repo path (project-local)
//   - key: dot-notation path, e.g. "preferences.language" or "project.stack"
//   - value: free-text or structured (no schema enforcement)
//   - weight: 1-10, used to prioritize entries when token budget is tight
//   - created_at / updated_at

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub scope: String,    // "global" | sha256(repo_path)
    pub key: String,
    pub value: String,
    pub weight: i64,      // 1-10 (10 = highest priority)
    pub source: String,   // "user" | "auto" | "pack:<name>"
    pub created_at: i64,  // unix seconds
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMemoryRequest {
    pub scope: String,
    pub key: String,
    pub value: String,
    pub weight: Option<i64>,
    pub source: Option<String>,
}

// ─── Store ────────────────────────────────────────────────────────────────────

pub struct MemoryStore {
    pool: SqlitePool,
}

impl MemoryStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Run migrations for the memory table (idempotent).
    pub async fn migrate(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS memory_entries (
                id         TEXT PRIMARY KEY,
                scope      TEXT NOT NULL DEFAULT 'global',
                key        TEXT NOT NULL,
                value      TEXT NOT NULL,
                weight     INTEGER NOT NULL DEFAULT 5,
                source     TEXT NOT NULL DEFAULT 'user',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(scope, key)
            );
            CREATE INDEX IF NOT EXISTS idx_memory_scope ON memory_entries(scope);
            CREATE INDEX IF NOT EXISTS idx_memory_weight ON memory_entries(weight DESC);
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Creating memory_entries table")?;
        Ok(())
    }

    /// List memory entries for a scope, sorted by weight descending.
    pub async fn list(&self, scope: &str) -> Result<Vec<MemoryEntry>> {
        let rows = sqlx::query_as!(
            MemoryEntry,
            r#"
            SELECT id, scope, key, value, weight, source, created_at, updated_at
            FROM memory_entries
            WHERE scope = ?
            ORDER BY weight DESC, updated_at DESC
            "#,
            scope
        )
        .fetch_all(&self.pool)
        .await
        .context("Fetching memory entries")?;
        Ok(rows)
    }

    /// List all entries (global + project scope combined).
    pub async fn list_all(&self, project_scope: &str) -> Result<Vec<MemoryEntry>> {
        let rows = sqlx::query_as!(
            MemoryEntry,
            r#"
            SELECT id, scope, key, value, weight, source, created_at, updated_at
            FROM memory_entries
            WHERE scope = 'global' OR scope = ?
            ORDER BY weight DESC, updated_at DESC
            "#,
            project_scope
        )
        .fetch_all(&self.pool)
        .await
        .context("Fetching all memory entries")?;
        Ok(rows)
    }

    /// Add or update a memory entry.
    pub async fn upsert(&self, req: AddMemoryRequest) -> Result<MemoryEntry> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let weight = req.weight.unwrap_or(5).clamp(1, 10);
        let source = req.source.unwrap_or_else(|| "user".to_string());

        sqlx::query!(
            r#"
            INSERT INTO memory_entries (id, scope, key, value, weight, source, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(scope, key) DO UPDATE SET
                value = excluded.value,
                weight = excluded.weight,
                source = excluded.source,
                updated_at = excluded.updated_at
            "#,
            id,
            req.scope,
            req.key,
            req.value,
            weight,
            source,
            now,
            now
        )
        .execute(&self.pool)
        .await
        .context("Upserting memory entry")?;

        // Return the final row (may have had a conflict — fetch by scope+key)
        let entry = sqlx::query_as!(
            MemoryEntry,
            r#"
            SELECT id, scope, key, value, weight, source, created_at, updated_at
            FROM memory_entries WHERE scope = ? AND key = ?
            "#,
            req.scope,
            req.key
        )
        .fetch_one(&self.pool)
        .await
        .context("Fetching upserted entry")?;

        Ok(entry)
    }

    /// Remove a memory entry by ID.
    pub async fn remove(&self, id: &str) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM memory_entries WHERE id = ?", id)
            .execute(&self.pool)
            .await
            .context("Deleting memory entry")?;
        Ok(result.rows_affected() > 0)
    }

    /// Get a single entry by scope + key.
    pub async fn get(&self, scope: &str, key: &str) -> Result<Option<MemoryEntry>> {
        let entry = sqlx::query_as!(
            MemoryEntry,
            r#"
            SELECT id, scope, key, value, weight, source, created_at, updated_at
            FROM memory_entries WHERE scope = ? AND key = ?
            "#,
            scope,
            key
        )
        .fetch_optional(&self.pool)
        .await
        .context("Fetching memory entry")?;
        Ok(entry)
    }

    /// Scope string for a project path: "global" or sha256(path).
    pub fn project_scope(repo_path: &str) -> String {
        if repo_path.is_empty() {
            return "global".to_string();
        }
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(repo_path.as_bytes());
        format!("proj:{}", hex::encode(&hash[..16]))
    }
}
