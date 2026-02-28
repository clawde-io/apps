// SPDX-License-Identifier: MIT
// Arena Mode — SQLite storage layer (Sprint K, AM.T01–AM.T03).

use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::model::{ArenaSession, ArenaVote, LeaderboardEntry};

/// Thin storage wrapper for arena tables.
///
/// Shares the daemon's main SQLite pool — no separate connection needed.
pub struct ArenaStorage {
    pool: SqlitePool,
}

impl ArenaStorage {
    /// Create a new `ArenaStorage` backed by the given pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ─── arena_sessions ─────────────────────────────────────────────────────

    /// Insert a new arena session record and return it.
    pub async fn create_session(
        &self,
        session_a_id: &str,
        session_b_id: &str,
        provider_a: &str,
        provider_b: &str,
        prompt: &str,
    ) -> Result<ArenaSession> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO arena_sessions
             (id, session_a_id, session_b_id, provider_a, provider_b, prompt, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(session_a_id)
        .bind(session_b_id)
        .bind(provider_a)
        .bind(provider_b)
        .bind(prompt)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.get_session(&id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("arena session not found after insert"))
    }

    /// Fetch an arena session by ID.
    pub async fn get_session(&self, id: &str) -> Result<Option<ArenaSession>> {
        Ok(sqlx::query_as("SELECT * FROM arena_sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    // ─── arena_votes ─────────────────────────────────────────────────────────

    /// Record a vote for an arena session and return the vote row.
    ///
    /// Enforces one-vote-per-arena by rejecting duplicates with a descriptive
    /// error rather than silently ignoring them.
    pub async fn record_vote(
        &self,
        arena_id: &str,
        winner_provider: &str,
        task_type: &str,
    ) -> Result<ArenaVote> {
        // Verify the arena session exists before inserting.
        self.get_session(arena_id).await?.ok_or_else(|| {
            anyhow::anyhow!("ARENA_NOT_FOUND: arena session '{arena_id}' not found")
        })?;

        // Check for duplicate vote.
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM arena_votes WHERE arena_id = ?")
                .bind(arena_id)
                .fetch_optional(&self.pool)
                .await?;
        if existing.is_some() {
            anyhow::bail!("ARENA_ALREADY_VOTED: arena session '{arena_id}' already has a vote");
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO arena_votes (id, arena_id, winner_provider, task_type, voted_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(arena_id)
        .bind(winner_provider)
        .bind(task_type)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(ArenaVote {
            id,
            arena_id: arena_id.to_string(),
            winner_provider: winner_provider.to_string(),
            task_type: task_type.to_string(),
            voted_at: now,
        })
    }

    // ─── leaderboard ─────────────────────────────────────────────────────────

    /// Return win-rate rankings aggregated by provider and task_type.
    ///
    /// When `task_type` is `Some`, only that task category is included.
    /// When `None`, all categories are included and an aggregate "all" row
    /// is appended per provider.
    pub async fn get_leaderboard(&self, task_type: Option<&str>) -> Result<Vec<LeaderboardEntry>> {
        // Per-task-type breakdown rows.
        let breakdown: Vec<(String, String, i64, i64)> = if let Some(tt) = task_type {
            sqlx::query_as(
                "SELECT
                     winner_provider,
                     task_type,
                     COUNT(*) AS wins,
                     (SELECT COUNT(*) FROM arena_votes v2
                      WHERE v2.task_type = arena_votes.task_type) AS total
                 FROM arena_votes
                 WHERE task_type = ?
                 GROUP BY winner_provider, task_type
                 ORDER BY wins DESC",
            )
            .bind(tt)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT
                     winner_provider,
                     task_type,
                     COUNT(*) AS wins,
                     (SELECT COUNT(*) FROM arena_votes v2
                      WHERE v2.task_type = arena_votes.task_type) AS total
                 FROM arena_votes
                 GROUP BY winner_provider, task_type
                 ORDER BY task_type, wins DESC",
            )
            .fetch_all(&self.pool)
            .await?
        };

        let mut entries: Vec<LeaderboardEntry> = breakdown
            .into_iter()
            .map(|(provider, tt, wins, total)| {
                let wins_u = wins as u64;
                let total_u = total.max(1) as u64;
                LeaderboardEntry {
                    provider,
                    task_type: tt,
                    wins: wins_u,
                    total: total_u,
                    win_rate: wins_u as f64 / total_u as f64,
                }
            })
            .collect();

        // When not filtering by task_type, append aggregate "all" rows.
        if task_type.is_none() {
            let aggregates: Vec<(String, i64, i64)> = sqlx::query_as(
                "SELECT
                     winner_provider,
                     COUNT(*) AS wins,
                     (SELECT COUNT(*) FROM arena_votes) AS total
                 FROM arena_votes
                 GROUP BY winner_provider
                 ORDER BY wins DESC",
            )
            .fetch_all(&self.pool)
            .await?;

            for (provider, wins, total) in aggregates {
                let wins_u = wins as u64;
                let total_u = total.max(1) as u64;
                entries.push(LeaderboardEntry {
                    provider,
                    task_type: "all".to_string(),
                    wins: wins_u,
                    total: total_u,
                    win_rate: wins_u as f64 / total_u as f64,
                });
            }
        }

        Ok(entries)
    }

    /// Return the total number of votes recorded across all arena sessions.
    pub async fn get_vote_count(&self) -> Result<u64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM arena_votes")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0 as u64)
    }
}
