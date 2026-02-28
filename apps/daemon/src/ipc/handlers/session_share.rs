//! Sprint EE CS.4/CS.5/CS.6 — `session.share*` RPC handlers.
//!
//! Cloud-tier session sharing. The daemon issues signed share tokens (JWT-style)
//! that allow other clients to join a session as read-only viewers or co-pilots.
//!
//! The relay proxies push events to all connected shareholders.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use uuid::Uuid;

/// `session.share` — Create a share token for a session.
///
/// Params:
/// ```json
/// {
///   "sessionId": "...",
///   "teamId":    "..." (optional),
///   "allowSend": false (optional, default false)
/// }
/// ```
pub async fn share(params: Value, ctx: &AppContext) -> Result<Value> {
    use sqlx::Row as _;

    let session_id = params
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("sessionId required"))?;
    let team_id = params.get("teamId").and_then(|v| v.as_str());
    let allow_send = params
        .get("allowSend")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Verify session exists
    let exists: bool = sqlx::query("SELECT 1 FROM sessions WHERE id = ?")
        .bind(session_id)
        .fetch_optional(ctx.storage.pool())
        .await?
        .is_some();

    if !exists {
        anyhow::bail!("Session '{}' not found", session_id);
    }

    let share_token = Uuid::new_v4().to_string();
    let id = Uuid::new_v4().to_string();
    // 8-hour TTL
    let ttl_hours = 8i64;

    sqlx::query(
        "INSERT INTO session_shares (id, session_id, share_token, team_id, allow_send, expires_at)
         VALUES (?, ?, ?, ?, ?, datetime('now', ? || ' hours'))",
    )
    .bind(&id)
    .bind(session_id)
    .bind(&share_token)
    .bind(team_id)
    .bind(allow_send)
    .bind(ttl_hours)
    .execute(ctx.storage.pool())
    .await?;

    // Read back with computed expiry
    let row = sqlx::query("SELECT expires_at FROM session_shares WHERE id = ?")
        .bind(&id)
        .fetch_one(ctx.storage.pool())
        .await?;

    let expires_at: String = row.get("expires_at");

    Ok(json!({
        "shareToken": share_token,
        "sessionId": session_id,
        "teamId": team_id,
        "allowSend": allow_send,
        "expiresAt": expires_at,
        "shareholderCount": 0,
    }))
}

/// `session.revokeShare` — Revoke all share tokens for a session.
pub async fn revoke_share(params: Value, ctx: &AppContext) -> Result<Value> {
    let session_id = params
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("sessionId required"))?;

    let result = sqlx::query(
        "UPDATE session_shares SET revoked_at = datetime('now')
         WHERE session_id = ? AND revoked_at IS NULL",
    )
    .bind(session_id)
    .execute(ctx.storage.pool())
    .await?;

    Ok(json!({
        "sessionId": session_id,
        "revokedCount": result.rows_affected(),
    }))
}

/// `session.shareList` — List active share tokens for a session.
pub async fn share_list(params: Value, ctx: &AppContext) -> Result<Value> {
    use sqlx::Row as _;

    let session_id = params
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("sessionId required"))?;

    let rows = sqlx::query(
        "SELECT share_token, team_id, allow_send, expires_at
         FROM session_shares
         WHERE session_id = ?
           AND revoked_at IS NULL
           AND expires_at > datetime('now')
         ORDER BY expires_at DESC",
    )
    .bind(session_id)
    .fetch_all(ctx.storage.pool())
    .await?;

    let shares: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "shareToken": r.get::<String, _>("share_token"),
                "teamId": r.get::<Option<String>, _>("team_id"),
                "allowSend": r.get::<bool, _>("allow_send"),
                "expiresAt": r.get::<String, _>("expires_at"),
                "shareholderCount": 0, // live count from relay — not stored
            })
        })
        .collect();

    Ok(json!({
        "sessionId": session_id,
        "shares": shares,
    }))
}
