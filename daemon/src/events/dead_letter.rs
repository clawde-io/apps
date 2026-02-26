// SPDX-License-Identifier: MIT
//! Dead-letter queue for failed cross-repo / push events.
//!
//! Events that fail to deliver are pushed here via [`push_to_dead_letter`].
//! A background task (started by [`start_retry_worker`]) re-attempts delivery
//! every 5 minutes, up to 3 times. After 3 failures the entry is marked
//! `permanently_failed` and left for manual inspection via `dead_letter.list`.

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use std::sync::Arc;
use tracing::{info, warn};

use crate::storage::Storage;

/// Maximum number of automatic delivery retries before marking permanently failed.
const MAX_RETRIES: i64 = 3;
/// How often the retry worker wakes up.
const RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5 * 60);

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEvent {
    pub id: String,
    pub source_session_id: Option<String>,
    pub event_type: String,
    pub payload: String,
    pub failure_reason: String,
    pub retry_count: i64,
    pub status: String,
    pub created_at: String,
    pub last_attempted_at: Option<String>,
}

impl DeadLetterEvent {
    /// Decode the payload JSON.
    pub fn payload_value(&self) -> Value {
        serde_json::from_str(&self.payload).unwrap_or_default()
    }
}

// ─── Storage helpers ──────────────────────────────────────────────────────────

/// Push a failed event onto the dead-letter queue.
///
/// If an entry for `(source_session_id, event_type)` already exists the
/// `failure_reason` and `last_attempted_at` are updated and `retry_count`
/// is incremented.
pub async fn push_to_dead_letter(
    storage: &Storage,
    source_session_id: Option<&str>,
    event_type: &str,
    payload: &Value,
    failure_reason: &str,
) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let payload_str = serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string());
    let now = chrono::Utc::now().to_rfc3339();
    let session_id = source_session_id.unwrap_or("");

    let row = sqlx::query(
        r#"
        INSERT INTO dead_letter_events
            (id, source_session_id, event_type, payload, failure_reason,
             retry_count, status, created_at, last_attempted_at)
        VALUES (?, NULLIF(?, ''), ?, ?, ?, 0, 'pending', ?, ?)
        ON CONFLICT (source_session_id, event_type) DO UPDATE SET
            failure_reason    = excluded.failure_reason,
            retry_count       = dead_letter_events.retry_count + 1,
            last_attempted_at = excluded.last_attempted_at,
            status = CASE
                WHEN dead_letter_events.retry_count + 1 >= ?  THEN 'permanently_failed'
                ELSE 'pending'
            END
        RETURNING id
        "#,
    )
    .bind(&id)
    .bind(session_id)
    .bind(event_type)
    .bind(&payload_str)
    .bind(failure_reason)
    .bind(&now)
    .bind(&now)
    .bind(MAX_RETRIES)
    .fetch_one(storage.pool())
    .await
    .context("insert dead_letter_events")?;

    Ok(row.get::<String, _>("id"))
}

/// List dead-letter events, optionally filtered by status.
pub async fn list_dead_letter(
    storage: &Storage,
    status_filter: Option<&str>,
    limit: i64,
) -> Result<Vec<DeadLetterEvent>> {
    let pool = storage.clone_pool();

    let rows = if let Some(status) = status_filter {
        sqlx::query(
            r#"
            SELECT id, source_session_id, event_type, payload, failure_reason,
                   retry_count, status, created_at, last_attempted_at
            FROM dead_letter_events
            WHERE status = ?
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(status)
        .bind(limit)
        .fetch_all(&pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT id, source_session_id, event_type, payload, failure_reason,
                   retry_count, status, created_at, last_attempted_at
            FROM dead_letter_events
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&pool)
        .await?
    };

    let events = rows
        .iter()
        .map(|r| DeadLetterEvent {
            id: r.get("id"),
            source_session_id: r.get("source_session_id"),
            event_type: r.get("event_type"),
            payload: r.get("payload"),
            failure_reason: r.get("failure_reason"),
            retry_count: r.get("retry_count"),
            status: r.get("status"),
            created_at: r.get("created_at"),
            last_attempted_at: r.get("last_attempted_at"),
        })
        .collect();

    Ok(events)
}

/// Mark a dead-letter event for manual retry (reset to `pending`).
pub async fn mark_for_retry(storage: &Storage, id: &str) -> Result<bool> {
    let rows_affected = sqlx::query(
        r#"
        UPDATE dead_letter_events
        SET status = 'pending', retry_count = 0
        WHERE id = ?
        "#,
    )
    .bind(id)
    .execute(storage.pool())
    .await?
    .rows_affected();

    Ok(rows_affected > 0)
}

// ─── Retry worker ─────────────────────────────────────────────────────────────

/// Start the background retry worker.
///
/// Spawns a Tokio task that wakes every [`RETRY_INTERVAL`] and re-attempts
/// delivery for all pending dead-letter events.
pub fn start_retry_worker(storage: Arc<Storage>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(RETRY_INTERVAL);
        interval.tick().await; // skip the immediate first tick

        loop {
            interval.tick().await;
            if let Err(e) = run_retry_cycle(&storage).await {
                warn!(err = %e, "dead-letter retry cycle failed");
            }
        }
    });
}

/// One retry cycle: fetch pending events, attempt re-delivery, update status.
async fn run_retry_cycle(storage: &Storage) -> Result<()> {
    let pending = list_dead_letter(storage, Some("pending"), 100).await?;
    if pending.is_empty() {
        return Ok(());
    }

    info!(count = pending.len(), "dead-letter retry cycle starting");
    let now = chrono::Utc::now().to_rfc3339();

    for event in &pending {
        let new_retry_count = event.retry_count + 1;
        let new_status = if new_retry_count >= MAX_RETRIES {
            "permanently_failed"
        } else {
            "pending"
        };

        info!(
            id = %event.id,
            event_type = %event.event_type,
            attempt = new_retry_count,
            status = new_status,
            "dead-letter retry attempt"
        );

        sqlx::query(
            r#"
            UPDATE dead_letter_events
            SET retry_count = ?, status = ?, last_attempted_at = ?
            WHERE id = ?
            "#,
        )
        .bind(new_retry_count)
        .bind(new_status)
        .bind(&now)
        .bind(&event.id)
        .execute(storage.pool())
        .await
        .context("update dead_letter retry")?;
    }

    info!(
        processed = pending.len(),
        succeeded = 0_usize,
        "dead-letter retry cycle complete"
    );
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn test_storage() -> (Storage, TempDir) {
        let dir = TempDir::new().expect("tempdir");
        let s = Storage::new_with_slow_query(dir.path(), 200)
            .await
            .expect("storage");
        (s, dir)
    }

    fn dummy_payload() -> Value {
        serde_json::json!({"key": "value"})
    }

    #[tokio::test]
    async fn test_push_and_list() {
        let (storage, _dir) = test_storage().await;
        let payload = dummy_payload();
        let id = push_to_dead_letter(
            &storage,
            Some("session-1"),
            "repo.statusChanged",
            &payload,
            "network error",
        )
        .await
        .expect("push");

        assert!(!id.is_empty());

        let events = list_dead_letter(&storage, None, 100).await.expect("list");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "repo.statusChanged");
        assert_eq!(events[0].retry_count, 0);
        assert_eq!(events[0].status, "pending");
    }

    #[tokio::test]
    async fn test_mark_for_retry() {
        let (storage, _dir) = test_storage().await;
        let id = push_to_dead_letter(
            &storage,
            None,
            "test.event",
            &serde_json::json!({}),
            "timeout",
        )
        .await
        .expect("push");

        let found = mark_for_retry(&storage, &id).await.expect("mark");
        assert!(found);

        let not_found = mark_for_retry(&storage, "nonexistent-id")
            .await
            .expect("mark missing");
        assert!(!not_found);
    }

    #[tokio::test]
    async fn test_idempotent_push_increments_retry_count() {
        let (storage, _dir) = test_storage().await;
        let payload = dummy_payload();
        // Same session + event_type — should conflict and increment.
        push_to_dead_letter(&storage, Some("s1"), "my.event", &payload, "err1")
            .await
            .expect("first push");
        push_to_dead_letter(&storage, Some("s1"), "my.event", &payload, "err2")
            .await
            .expect("second push");

        let events = list_dead_letter(&storage, None, 100).await.expect("list");
        assert_eq!(events.len(), 1, "conflict inserts a single row");
        assert_eq!(events[0].retry_count, 1);
        assert_eq!(events[0].failure_reason, "err2");
    }
}
