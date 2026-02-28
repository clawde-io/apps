//! Sprint CC AM.2 — Attention tracker middleware for tool calls.
//!
//! Tracks which files a session reads, writes, and mentions during its
//! lifetime. Counts are persisted to `session_file_attention` (migration 042).
//!
//! ## Usage
//!
//! Call [`record_file_access`] from tool call handlers whenever a file
//! operation occurs. The function upserts into the attention table.

use anyhow::Result;
use sqlx::SqlitePool;

/// The type of file access to record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessKind {
    Read,
    Write,
    Mention,
}

/// Record a file access event for a session.
///
/// Upserts a row in `session_file_attention`, incrementing the appropriate
/// counter. Safe to call from any async context — uses a short-lived query.
pub async fn record_file_access(
    pool: &SqlitePool,
    session_id: &str,
    file_path: &str,
    kind: AccessKind,
) -> Result<()> {
    match kind {
        AccessKind::Read => {
            sqlx::query(
                "INSERT INTO session_file_attention (session_id, file_path, read_count)
                 VALUES (?, ?, 1)
                 ON CONFLICT (session_id, file_path)
                 DO UPDATE SET read_count = read_count + 1",
            )
            .bind(session_id)
            .bind(file_path)
            .execute(pool)
            .await?;
        }
        AccessKind::Write => {
            sqlx::query(
                "INSERT INTO session_file_attention (session_id, file_path, write_count)
                 VALUES (?, ?, 1)
                 ON CONFLICT (session_id, file_path)
                 DO UPDATE SET write_count = write_count + 1",
            )
            .bind(session_id)
            .bind(file_path)
            .execute(pool)
            .await?;
        }
        AccessKind::Mention => {
            sqlx::query(
                "INSERT INTO session_file_attention (session_id, file_path, mention_count)
                 VALUES (?, ?, 1)
                 ON CONFLICT (session_id, file_path)
                 DO UPDATE SET mention_count = mention_count + 1",
            )
            .bind(session_id)
            .bind(file_path)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

/// Fetch the attention map for a session — sorted by total access count desc.
pub async fn get_attention_map(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<FileAttentionEntry>> {
    let rows = sqlx::query_as::<_, FileAttentionEntry>(
        "SELECT file_path, read_count, write_count, mention_count
         FROM session_file_attention
         WHERE session_id = ?
         ORDER BY (read_count + write_count + mention_count) DESC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// A single entry in the attention map.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FileAttentionEntry {
    pub file_path: String,
    pub read_count: i64,
    pub write_count: i64,
    pub mention_count: i64,
}

impl FileAttentionEntry {
    /// Total access count across all kinds.
    pub fn total(&self) -> i64 {
        self.read_count + self.write_count + self.mention_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_kind_variants() {
        // Ensure all variants are reachable without panicking.
        let kinds = [AccessKind::Read, AccessKind::Write, AccessKind::Mention];
        assert_eq!(kinds.len(), 3);
    }

    #[test]
    fn test_total_calculation() {
        let entry = FileAttentionEntry {
            file_path: "src/main.rs".to_string(),
            read_count: 3,
            write_count: 2,
            mention_count: 5,
        };
        assert_eq!(entry.total(), 10);
    }
}
