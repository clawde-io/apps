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
        let opts = SqliteConnectOptions::from_str(&format!(
            "sqlite://{}?mode=rwc",
            db_path.display()
        ))?
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .create_if_missing(true);

        let pool = SqlitePool::connect_with(opts).await?;
        Self::migrate(&pool).await?;
        Ok(Self { pool })
    }

    async fn migrate(pool: &SqlitePool) -> Result<()> {
        let sql = include_str!("migrations/001_init.sql");
        // Execute each statement separately
        for stmt in sql.split(';') {
            let stmt = stmt.trim();
            if !stmt.is_empty() {
                sqlx::query(stmt).execute(pool).await?;
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
        self.get_session(&id).await?.ok_or_else(|| anyhow::anyhow!("session not found after insert"))
    }

    pub async fn get_session(&self, id: &str) -> Result<Option<SessionRow>> {
        Ok(sqlx::query_as("SELECT * FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionRow>> {
        Ok(sqlx::query_as("SELECT * FROM sessions ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?)
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

    pub async fn update_message_content(&self, id: &str, content: &str, status: &str) -> Result<()> {
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
        let rows = if let Some(cursor) = before {
            sqlx::query_as(
                "SELECT * FROM messages WHERE session_id = ? AND created_at < ?
                 ORDER BY created_at DESC LIMIT ?",
            )
            .bind(session_id)
            .bind(cursor)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT * FROM messages WHERE session_id = ? ORDER BY created_at ASC LIMIT ?",
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
        sqlx::query(
            "UPDATE tool_calls SET output = ?, status = ?, completed_at = ? WHERE id = ?",
        )
        .bind(output)
        .bind(status)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_tool_calls_for_session(&self, session_id: &str) -> Result<Vec<ToolCallRow>> {
        Ok(sqlx::query_as(
            "SELECT * FROM tool_calls WHERE session_id = ? ORDER BY created_at ASC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?)
    }

    // ─── Settings ───────────────────────────────────────────────────────────

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM settings WHERE key = ?")
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
}
