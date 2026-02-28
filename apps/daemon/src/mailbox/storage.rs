// SPDX-License-Identifier: MIT
// Sprint N — Mailbox SQLite storage (MR.T06, MR.T08, MR.T09).

use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::model::MailboxMessage;

// ─── Raw DB row ───────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: String,
    from_repo: String,
    to_repo: String,
    subject: String,
    body: String,
    reply_to: Option<String>,
    expires_at: Option<String>,
    archived: i64,
    created_at: String,
}

impl From<MessageRow> for MailboxMessage {
    fn from(r: MessageRow) -> MailboxMessage {
        MailboxMessage {
            id: r.id,
            from_repo: r.from_repo,
            to_repo: r.to_repo,
            subject: r.subject,
            body: r.body,
            reply_to: r.reply_to,
            expires_at: r.expires_at,
            archived: r.archived != 0,
            created_at: r.created_at,
        }
    }
}

// ─── MailboxStorage ───────────────────────────────────────────────────────────

/// SQLite-backed storage for cross-repo mailbox messages.
#[derive(Clone)]
pub struct MailboxStorage {
    pool: SqlitePool,
}

impl MailboxStorage {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ── Mutation ──────────────────────────────────────────────────────────────

    /// Persist a new message.  Uses IGNORE so duplicate IDs are silently
    /// dropped (idempotent — safe to call if the watcher fires twice).
    pub async fn send_message(
        &self,
        from_repo: &str,
        to_repo: &str,
        subject: &str,
        body: &str,
        reply_to: Option<&str>,
        expires_at: Option<&str>,
    ) -> Result<MailboxMessage> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT OR IGNORE INTO mailbox_messages
                 (id, from_repo, to_repo, subject, body, reply_to, expires_at, archived, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(&id)
        .bind(from_repo)
        .bind(to_repo)
        .bind(subject)
        .bind(body)
        .bind(reply_to)
        .bind(expires_at)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        let row: MessageRow = sqlx::query_as("SELECT * FROM mailbox_messages WHERE id = ?")
            .bind(&id)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.into())
    }

    /// Insert a message with a pre-assigned id (used when importing from a file).
    pub async fn insert_with_id(&self, msg: &MailboxMessage) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO mailbox_messages
                 (id, from_repo, to_repo, subject, body, reply_to, expires_at, archived, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&msg.id)
        .bind(&msg.from_repo)
        .bind(&msg.to_repo)
        .bind(&msg.subject)
        .bind(&msg.body)
        .bind(msg.reply_to.as_deref())
        .bind(msg.expires_at.as_deref())
        .bind(if msg.archived { 1i64 } else { 0i64 })
        .bind(&msg.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Mark a message as archived (processed).
    pub async fn archive_message(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE mailbox_messages SET archived = 1 WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Delete messages whose `expires_at` timestamp is in the past and move
    /// their on-disk file to `{to_repo}/.claude/archive/inbox/`.
    ///
    /// Returns the number of messages pruned.
    pub async fn prune_expired(&self) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        // Fetch expired, non-archived messages first so we can move their files.
        let expired: Vec<MessageRow> = sqlx::query_as(
            "SELECT * FROM mailbox_messages
             WHERE archived = 0
               AND expires_at IS NOT NULL
               AND expires_at < ?",
        )
        .bind(&now)
        .fetch_all(&self.pool)
        .await?;

        let count = expired.len() as u64;
        for row in &expired {
            // Move the inbox file to the dead-letter directory.
            move_to_dead_letter(&row.to_repo, &row.id);
        }

        // Delete from DB.
        sqlx::query(
            "DELETE FROM mailbox_messages
             WHERE archived = 0
               AND expires_at IS NOT NULL
               AND expires_at < ?",
        )
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(count)
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Return unarchived messages for `to_repo`, newest first.
    pub async fn list_messages(&self, to_repo: &str) -> Result<Vec<MailboxMessage>> {
        let rows: Vec<MessageRow> = sqlx::query_as(
            "SELECT * FROM mailbox_messages
             WHERE to_repo = ? AND archived = 0
             ORDER BY created_at DESC",
        )
        .bind(to_repo)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Return the count of unarchived messages for `to_repo`.
    pub async fn unread_count(&self, to_repo: &str) -> Result<u64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM mailbox_messages WHERE to_repo = ? AND archived = 0",
        )
        .bind(to_repo)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0 as u64)
    }
}

// ─── Dead-letter helper ───────────────────────────────────────────────────────

/// Move `{to_repo}/.claude/inbox/{id}.md` to
/// `{to_repo}/.claude/archive/inbox/{id}.md` (best-effort).
fn move_to_dead_letter(to_repo: &str, id: &str) {
    let inbox = std::path::Path::new(to_repo)
        .join(".claude/inbox")
        .join(format!("{id}.md"));
    let archive = std::path::Path::new(to_repo).join(".claude/archive/inbox");
    if inbox.is_file() && std::fs::create_dir_all(&archive).is_ok() {
        let dest = archive.join(format!("{id}.md"));
        let _ = std::fs::rename(&inbox, &dest);
    }
}
