// tasks/janitor.rs — Lease janitor background task (Sprint ZZ LH.T03)
//
// Runs every 30 seconds. Releases expired task leases back to 'open' status.
// A task's lease is expired when lease_expires_at < now() AND status = 'claimed'.

use crate::storage::Storage;
use anyhow::Result;
use std::sync::Arc;
use tracing::{info, warn};

/// Release tasks whose lease has expired.
///
/// Called every 30 seconds by the background janitor loop.
pub async fn release_expired_leases(storage: &Storage) -> Result<usize> {
    let now = chrono::Utc::now().timestamp();

    // Find expired leases
    let expired: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT id, claimed_by_agent_id FROM agent_tasks \
         WHERE status = 'claimed' \
           AND lease_expires_at IS NOT NULL \
           AND lease_expires_at < ?",
    )
    .bind(now)
    .fetch_all(storage.pool())
    .await?;

    if expired.is_empty() {
        return Ok(0);
    }

    let count = expired.len();
    for (task_id, agent_id) in &expired {
        info!(
            task_id = %task_id,
            agent = ?agent_id,
            "lease expired — releasing task back to open"
        );

        let result = sqlx::query(
            "UPDATE agent_tasks \
             SET status = 'open', \
                 claimed_by = NULL, \
                 claimed_by_agent_id = NULL, \
                 lease_expires_at = NULL, \
                 last_heartbeat_at = NULL, \
                 updated_at = ? \
             WHERE id = ? AND status = 'claimed'",
        )
        .bind(now)
        .bind(task_id)
        .execute(storage.pool())
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => {
                // Log the lease expiration event
                let event_id = uuid::Uuid::new_v4().to_string().replace('-', "");
                let _ = sqlx::query(
                    "INSERT INTO task_activity_log \
                     (id, task_id, agent_id, action, detail, created_at) \
                     VALUES (?, ?, ?, 'lease_expired', 'Lease expired — task released to open', ?)",
                )
                .bind(&event_id)
                .bind(task_id)
                .bind(agent_id.as_deref().unwrap_or("system"))
                .bind(now)
                .execute(storage.pool())
                .await;
            }
            Ok(_) => {
                // Task was already released by someone else — no action needed
            }
            Err(e) => {
                warn!(task_id = %task_id, err = %e, "failed to release expired lease");
            }
        }
    }

    if count > 0 {
        info!(count, "lease janitor released expired task leases");
    }

    Ok(count)
}

/// Background janitor task — runs perpetually, releasing expired leases every 30s.
///
/// Call this in a `tokio::spawn` during daemon startup.
pub async fn run_lease_janitor(storage: Arc<Storage>) {
    info!("lease janitor started (30s interval)");
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));

    loop {
        interval.tick().await;
        match release_expired_leases(&storage).await {
            Ok(n) if n > 0 => info!(released = n, "lease janitor: released expired tasks"),
            Ok(_) => {}
            Err(e) => warn!(err = %e, "lease janitor error"),
        }
    }
}

/// LH.T02 — Extend a task's lease by `extend_secs` seconds from now.
///
/// Called by `task.heartbeat` RPC handler.
pub async fn extend_lease(storage: &Storage, task_id: &str, extend_secs: i64) -> Result<i64> {
    let now = chrono::Utc::now().timestamp();
    let new_expires = now + extend_secs;

    let rows_affected = sqlx::query(
        "UPDATE agent_tasks \
         SET lease_expires_at = ?, last_heartbeat_at = ? \
         WHERE id = ? AND status = 'claimed'",
    )
    .bind(new_expires)
    .bind(now)
    .bind(task_id)
    .execute(storage.pool())
    .await?
    .rows_affected();

    if rows_affected == 0 {
        return Err(anyhow::anyhow!(
            "task '{task_id}' is not currently claimed — cannot extend lease"
        ));
    }

    Ok(new_expires)
}

/// LH.T04 — Atomic claim with lease: claim a task iff status = 'open' AND
/// (lease_expires_at IS NULL OR lease_expires_at < now()).
///
/// Returns the task ID if claimed, or None if already taken.
pub async fn atomic_claim_with_lease(
    storage: &Storage,
    task_id: &str,
    agent_id: &str,
    lease_secs: i64,
) -> Result<bool> {
    let now = chrono::Utc::now().timestamp();
    let lease_expires = now + lease_secs;

    let rows_affected = sqlx::query(
        "UPDATE agent_tasks \
         SET status = 'claimed', \
             claimed_by = ?, \
             claimed_by_agent_id = ?, \
             lease_expires_at = ?, \
             last_heartbeat_at = ?, \
             updated_at = ? \
         WHERE id = ? \
           AND status = 'open' \
           AND (lease_expires_at IS NULL OR lease_expires_at < ?)",
    )
    .bind(agent_id)
    .bind(agent_id)
    .bind(lease_expires)
    .bind(now)
    .bind(now)
    .bind(task_id)
    .bind(now)
    .execute(storage.pool())
    .await?
    .rows_affected();

    Ok(rows_affected > 0)
}
