// ipc/handlers/pack_ratings.rs — Pack rating RPC (Sprint TT SK.1).
//
// RPC: pack.rate
//   params: { pack_slug, rating: 1-5 }
//   result: { avg_rating, total_ratings }

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn rate(ctx: Arc<AppContext>, params: Value) -> Result<Value> {
    let pack_slug = params["pack_slug"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing pack_slug"))?;
    let rating = params["rating"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("missing rating"))?;

    if !(1..=5).contains(&rating) {
        return Err(anyhow::anyhow!("rating must be between 1 and 5"));
    }

    // Use daemon_id as user identifier for local ratings
    let user_id = &ctx.daemon_id;

    sqlx::query(
        "INSERT INTO pack_ratings (pack_slug, user_id, rating)
         VALUES (?, ?, ?)
         ON CONFLICT(pack_slug, user_id) DO UPDATE SET
           rating = excluded.rating,
           updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
    )
    .bind(pack_slug)
    .bind(user_id)
    .bind(rating as i32)
    .execute(ctx.storage.pool())
    .await?;

    // Compute updated average
    let row: (f64, i64) = sqlx::query_as(
        "SELECT AVG(CAST(rating AS REAL)), COUNT(*) FROM pack_ratings WHERE pack_slug = ?",
    )
    .bind(pack_slug)
    .fetch_one(ctx.storage.pool())
    .await?;

    Ok(json!({
        "avg_rating": (row.0 * 10.0).round() / 10.0,
        "total_ratings": row.1,
    }))
}
