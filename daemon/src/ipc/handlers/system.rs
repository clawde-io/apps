//! System resource handlers — Phase 44 (F44.6.2)
//!
//! Exposes current RAM/CPU stats and historical resource metrics over JSON-RPC 2.0.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use sqlx::FromRow;

/// Row type for reading the latest resource metrics snapshot.
#[derive(Debug, FromRow)]
struct LatestMetrics {
    total_ram_bytes: i64,
    used_ram_bytes: i64,
    daemon_ram_bytes: i64,
}

/// Row type for reading historical resource metrics.
#[derive(Debug, FromRow)]
struct MetricsRow {
    timestamp: i64,
    total_ram_bytes: i64,
    used_ram_bytes: i64,
    daemon_ram_bytes: i64,
    active_session_count: i64,
    warm_session_count: i64,
    cold_session_count: i64,
    pool_worker_count: i64,
    context_compressions: i64,
}

/// `system.resources` — return current RAM usage and session tier counts.
pub async fn resources(_params: Value, ctx: &AppContext) -> Result<Value> {
    let pool = ctx.storage.clone_pool();

    // Count sessions by tier
    let active_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE tier = 'active'")
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));

    let warm_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE tier = 'warm'")
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    let cold_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE tier = 'cold'")
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    // Get latest resource_metrics row if it exists
    let latest: Option<LatestMetrics> = sqlx::query_as(
        "SELECT total_ram_bytes, used_ram_bytes, daemon_ram_bytes \
         FROM resource_metrics ORDER BY timestamp DESC LIMIT 1",
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let (total_ram, used_ram, daemon_ram) = if let Some(row) = latest {
        (
            row.total_ram_bytes,
            row.used_ram_bytes,
            row.daemon_ram_bytes,
        )
    } else {
        (0i64, 0i64, 0i64)
    };

    let used_pct = if total_ram > 0 {
        (used_ram as f64 / total_ram as f64 * 100.0).round() as i64
    } else {
        0
    };

    Ok(json!({
        "ram": {
            "totalBytes": total_ram,
            "usedBytes": used_ram,
            "daemonBytes": daemon_ram,
            "usedPercent": used_pct,
        },
        "sessions": {
            "active": active_count.0,
            "warm": warm_count.0,
            "cold": cold_count.0,
        }
    }))
}

/// `system.resourceHistory` — return recent resource_metrics rows (last N).
pub async fn resource_history(params: Value, ctx: &AppContext) -> Result<Value> {
    let limit = params
        .get("limit")
        .and_then(Value::as_i64)
        .unwrap_or(60)
        .clamp(1, 1440); // max 24h at 1-min intervals

    let pool = ctx.storage.clone_pool();
    let rows: Vec<MetricsRow> = sqlx::query_as(
        "SELECT timestamp, total_ram_bytes, used_ram_bytes, daemon_ram_bytes, \
                active_session_count, warm_session_count, cold_session_count, \
                pool_worker_count, context_compressions \
         FROM resource_metrics \
         ORDER BY timestamp DESC \
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?;

    let entries: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "timestamp": r.timestamp,
                "totalRamBytes": r.total_ram_bytes,
                "usedRamBytes": r.used_ram_bytes,
                "daemonRamBytes": r.daemon_ram_bytes,
                "activeSessions": r.active_session_count,
                "warmSessions": r.warm_session_count,
                "coldSessions": r.cold_session_count,
                "poolWorkers": r.pool_worker_count,
                "contextCompressions": r.context_compressions,
            })
        })
        .collect();

    let count = entries.len();
    Ok(json!({ "history": entries, "count": count }))
}
