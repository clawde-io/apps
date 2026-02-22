use anyhow::Result;
use chrono::Utc;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::{path::Path, str::FromStr};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    pub provider: String,
    pub repo_path: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MessageRow {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LicenseCacheRow {
    pub id: i64,
    pub tier: String,
    pub features: String, // JSON string
    pub cached_at: String,
    pub valid_until: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AccountRow {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub credentials_path: String,
    pub priority: i64,
    pub limited_until: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ToolCallRow {
    pub id: String,
    pub message_id: String,
    pub session_id: String,
    pub name: String,
    pub input: String,
    pub output: Option<String>,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
}

impl Storage {
    pub async fn new(data_dir: &Path) -> Result<Self> {
        tokio::fs::create_dir_all(data_dir).await?;
        let db_path = data_dir.join("clawd.db");
        let opts =
            SqliteConnectOptions::from_str(&format!("sqlite://{}?mode=rwc", db_path.display()))?
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
                .create_if_missing(true);

        let pool = SqlitePool::connect_with(opts).await?;
        Self::migrate(&pool).await?;
        Ok(Self { pool })
    }

    async fn migrate(pool: &SqlitePool) -> Result<()> {
        for sql in [
            include_str!("migrations/001_init.sql"),
            include_str!("migrations/002_license.sql"),
        ] {
            for stmt in sql.split(';') {
                let stmt = stmt.trim();
                if !stmt.is_empty() {
                    sqlx::query(stmt).execute(pool).await?;
                }
            }
        }
        Ok(())
    }

    // ─── Sessions ───────────────────────────────────────────────────────────

    pub async fn create_session(
        &self,
        provider: &str,
        repo_path: &str,
        title: &str,
    ) -> Result<SessionRow> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO sessions (id, provider, repo_path, title, status, created_at, updated_at, message_count)
             VALUES (?, ?, ?, ?, 'idle', ?, ?, 0)",
        )
        .bind(&id)
        .bind(provider)
        .bind(repo_path)
        .bind(title)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_session(&id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("session not found after insert"))
    }

    pub async fn get_session(&self, id: &str) -> Result<Option<SessionRow>> {
        Ok(sqlx::query_as("SELECT * FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionRow>> {
        Ok(
            sqlx::query_as("SELECT * FROM sessions ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn update_session_status(&self, id: &str, status: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE sessions SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn increment_message_count(&self, session_id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE sessions SET message_count = message_count + 1, updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_session(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ─── Messages ───────────────────────────────────────────────────────────

    pub async fn create_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        status: &str,
    ) -> Result<MessageRow> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(session_id)
        .bind(role)
        .bind(content)
        .bind(status)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_message(&id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("message not found after insert"))
    }

    pub async fn get_message(&self, id: &str) -> Result<Option<MessageRow>> {
        Ok(sqlx::query_as("SELECT * FROM messages WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn update_message_content(
        &self,
        id: &str,
        content: &str,
        status: &str,
    ) -> Result<()> {
        sqlx::query("UPDATE messages SET content = ?, status = ? WHERE id = ?")
            .bind(content)
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_messages(
        &self,
        session_id: &str,
        limit: i64,
        before: Option<&str>,
    ) -> Result<Vec<MessageRow>> {
        // The `before` parameter is a message *ID*, not a timestamp.
        // We resolve it to a timestamp via a subquery so the pagination cursor
        // works correctly.  Results are always returned in chronological order
        // (oldest first) for the chat UI to render in the correct direction.
        let rows = if let Some(msg_id) = before {
            // Get the last `limit` messages older than the given message ID,
            // returned in ascending order for display.
            sqlx::query_as(
                "SELECT * FROM (
                     SELECT * FROM messages
                     WHERE session_id = ?
                       AND created_at < (SELECT created_at FROM messages WHERE id = ?)
                     ORDER BY created_at DESC LIMIT ?
                 ) ORDER BY created_at ASC",
            )
            .bind(session_id)
            .bind(msg_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            // Initial load: return the *last* `limit` messages in chronological
            // order so the chat view shows the most recent conversation.
            sqlx::query_as(
                "SELECT * FROM (
                     SELECT * FROM messages WHERE session_id = ?
                     ORDER BY created_at DESC LIMIT ?
                 ) ORDER BY created_at ASC",
            )
            .bind(session_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    // ─── Tool Calls ─────────────────────────────────────────────────────────

    pub async fn create_tool_call(
        &self,
        session_id: &str,
        message_id: &str,
        name: &str,
        input: &str,
    ) -> Result<ToolCallRow> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO tool_calls (id, message_id, session_id, name, input, status, created_at)
             VALUES (?, ?, ?, ?, ?, 'pending', ?)",
        )
        .bind(&id)
        .bind(message_id)
        .bind(session_id)
        .bind(name)
        .bind(input)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_tool_call(&id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("tool_call not found after insert"))
    }

    pub async fn get_tool_call(&self, id: &str) -> Result<Option<ToolCallRow>> {
        Ok(sqlx::query_as("SELECT * FROM tool_calls WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn complete_tool_call(
        &self,
        id: &str,
        output: Option<&str>,
        status: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE tool_calls SET output = ?, status = ?, completed_at = ? WHERE id = ?")
            .bind(output)
            .bind(status)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_tool_calls_for_session(&self, session_id: &str) -> Result<Vec<ToolCallRow>> {
        Ok(
            sqlx::query_as("SELECT * FROM tool_calls WHERE session_id = ? ORDER BY created_at ASC")
                .bind(session_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    // ─── Startup recovery ───────────────────────────────────────────────────

    /// On daemon startup, any session left in 'running' or 'waiting' state
    /// from a previous (crashed/killed) process is marked as 'error'.
    /// Returns the number of sessions recovered.
    pub async fn recover_stale_sessions(&self) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE sessions SET status = 'error', updated_at = ?
             WHERE status IN ('running', 'waiting')",
        )
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    // ─── Settings ───────────────────────────────────────────────────────────

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(v,)| v))
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ─── License cache ───────────────────────────────────────────────────────

    pub async fn get_license_cache(&self) -> Result<Option<LicenseCacheRow>> {
        Ok(sqlx::query_as("SELECT * FROM license_cache WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn set_license_cache(
        &self,
        tier: &str,
        features: &str,
        cached_at: &str,
        valid_until: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO license_cache (id, tier, features, cached_at, valid_until)
             VALUES (1, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               tier = excluded.tier,
               features = excluded.features,
               cached_at = excluded.cached_at,
               valid_until = excluded.valid_until",
        )
        .bind(tier)
        .bind(features)
        .bind(cached_at)
        .bind(valid_until)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ─── Accounts ────────────────────────────────────────────────────────────

    pub async fn list_accounts(&self) -> Result<Vec<AccountRow>> {
        Ok(
            sqlx::query_as("SELECT * FROM accounts ORDER BY priority ASC")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn create_account(
        &self,
        name: &str,
        provider: &str,
        credentials_path: &str,
        priority: i64,
    ) -> Result<AccountRow> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO accounts (id, name, provider, credentials_path, priority)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(provider)
        .bind(credentials_path)
        .bind(priority)
        .execute(&self.pool)
        .await?;
        self.get_account(&id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("account not found after insert"))
    }

    pub async fn get_account(&self, id: &str) -> Result<Option<AccountRow>> {
        Ok(sqlx::query_as("SELECT * FROM accounts WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn delete_account(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM accounts WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_account_limited(&self, id: &str, limited_until: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE accounts SET limited_until = ? WHERE id = ?")
            .bind(limited_until)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
