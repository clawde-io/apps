//! RPC handlers for the account scheduler.
//!
//! Exposes:
//!   `scheduler.status` — report scheduler state: accounts, queue depth, etc.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// `scheduler.status` — return a snapshot of the scheduler's current state.
///
/// Params: (none required)
/// Returns:
/// ```json
/// {
///   "accounts": [ { account_id, provider, is_available, blocked_until, rpm_used, tpm_used, total_requests } ],
///   "queue_length": N,
///   "queue_next_priority": N | null,
///   "fallback_config": { "primary": "...", "alternatives": [...] }
/// }
/// ```
pub async fn status(_params: Value, ctx: &AppContext) -> Result<Value> {
    let accounts = ctx.account_pool.list().await;
    let queue_length = ctx.scheduler_queue.len().await;
    let queue_next_priority = ctx.scheduler_queue.peek_priority().await;

    let accounts_json: Vec<Value> = accounts
        .into_iter()
        .map(|a| {
            json!({
                "account_id": a.account_id,
                "provider": a.provider,
                "vault_ref": a.vault_ref,
                "is_available": a.is_available,
                "blocked_until": a.blocked_until.map(|t| t.to_rfc3339()),
                "rpm_used": a.rpm_used,
                "tpm_used": a.tpm_used,
                "total_requests": a.total_requests,
                "last_used": a.last_used.map(|t| t.to_rfc3339()),
            })
        })
        .collect();

    Ok(json!({
        "accounts": accounts_json,
        "queue_length": queue_length,
        "queue_next_priority": queue_next_priority,
    }))
}
