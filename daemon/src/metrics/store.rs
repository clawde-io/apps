// metrics/store.rs — Session metrics storage + hourly rollups (Sprint PP OB.1).
//
// Stores per-tick metrics in `metrics` table and hourly rollups in
// `metric_rollups`. Used by cost dashboard, budget enforcement, and the
// Cloud metrics ingest pipeline.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEntry {
    pub id: i64,
    pub session_id: String,
    pub timestamp: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub tool_calls: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricRollup {
    pub id: i64,
    pub session_id: Option<String>,
    pub hour_bucket: i64, // Unix timestamp rounded to hour
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub tool_calls: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub total_tokens_in: i64,
    pub total_tokens_out: i64,
    pub total_tool_calls: i64,
    pub total_cost_usd: f64,
    pub session_count: i64,
    pub period_start: i64,
    pub period_end: i64,
}

pub struct MetricsStore {
    pool: SqlitePool,
}

impl MetricsStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                tokens_in INTEGER NOT NULL DEFAULT 0,
                tokens_out INTEGER NOT NULL DEFAULT 0,
                tool_calls INTEGER NOT NULL DEFAULT 0,
                cost_usd REAL NOT NULL DEFAULT 0.0
            );
            CREATE INDEX IF NOT EXISTS idx_metrics_session ON metrics(session_id);
            CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON metrics(timestamp);

            CREATE TABLE IF NOT EXISTS metric_rollups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT,
                hour_bucket INTEGER NOT NULL,
                tokens_in INTEGER NOT NULL DEFAULT 0,
                tokens_out INTEGER NOT NULL DEFAULT 0,
                tool_calls INTEGER NOT NULL DEFAULT 0,
                cost_usd REAL NOT NULL DEFAULT 0.0,
                UNIQUE(session_id, hour_bucket)
            );
            CREATE INDEX IF NOT EXISTS idx_rollups_hour ON metric_rollups(hour_bucket);
            ",
        )
        .execute(&self.pool)
        .await
        .context("Creating metrics tables")?;
        Ok(())
    }

    /// Record a metrics tick for a session.
    pub async fn record(
        &self,
        session_id: &str,
        tokens_in: i64,
        tokens_out: i64,
        tool_calls: i64,
        cost_usd: f64,
    ) -> Result<MetricEntry> {
        let now = now_secs();
        let id = sqlx::query_scalar::<_, i64>(
            r"INSERT INTO metrics (session_id, timestamp, tokens_in, tokens_out, tool_calls, cost_usd)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6)
              RETURNING id",
        )
        .bind(session_id)
        .bind(now)
        .bind(tokens_in)
        .bind(tokens_out)
        .bind(tool_calls)
        .bind(cost_usd)
        .fetch_one(&self.pool)
        .await
        .context("Inserting metric entry")?;

        // Upsert hourly rollup
        let hour = (now / 3600) * 3600;
        sqlx::query(
            r"INSERT INTO metric_rollups (session_id, hour_bucket, tokens_in, tokens_out, tool_calls, cost_usd)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6)
              ON CONFLICT(session_id, hour_bucket) DO UPDATE SET
                tokens_in = tokens_in + excluded.tokens_in,
                tokens_out = tokens_out + excluded.tokens_out,
                tool_calls = tool_calls + excluded.tool_calls,
                cost_usd = cost_usd + excluded.cost_usd",
        )
        .bind(session_id)
        .bind(hour)
        .bind(tokens_in)
        .bind(tokens_out)
        .bind(tool_calls)
        .bind(cost_usd)
        .execute(&self.pool)
        .await
        .context("Upserting metric rollup")?;

        Ok(MetricEntry {
            id,
            session_id: session_id.to_string(),
            timestamp: now,
            tokens_in,
            tokens_out,
            tool_calls,
            cost_usd,
        })
    }

    /// List recent metric entries for a session.
    pub async fn list_session(
        &self,
        session_id: &str,
        limit: i64,
    ) -> Result<Vec<MetricEntry>> {
        let rows = sqlx::query_as!(
            MetricEntry,
            r"SELECT id, session_id, timestamp, tokens_in, tokens_out, tool_calls, cost_usd
              FROM metrics WHERE session_id = ?1
              ORDER BY timestamp DESC LIMIT ?2",
            session_id,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .context("Listing session metrics")?;
        Ok(rows)
    }

    /// Aggregate summary over a time window (Unix seconds).
    pub async fn summary(&self, since: i64, until: i64) -> Result<MetricsSummary> {
        let row = sqlx::query!(
            r"SELECT
                COALESCE(SUM(tokens_in), 0) as total_tokens_in,
                COALESCE(SUM(tokens_out), 0) as total_tokens_out,
                COALESCE(SUM(tool_calls), 0) as total_tool_calls,
                COALESCE(SUM(cost_usd), 0.0) as total_cost_usd,
                COUNT(DISTINCT session_id) as session_count
              FROM metrics WHERE timestamp >= ?1 AND timestamp <= ?2",
            since,
            until,
        )
        .fetch_one(&self.pool)
        .await
        .context("Querying metrics summary")?;

        Ok(MetricsSummary {
            total_tokens_in: row.total_tokens_in,
            total_tokens_out: row.total_tokens_out,
            total_tool_calls: row.total_tool_calls,
            total_cost_usd: row.total_cost_usd,
            session_count: row.session_count,
            period_start: since,
            period_end: until,
        })
    }

    /// Rolling daily cost (last 24h) in USD.
    pub async fn daily_cost_usd(&self) -> Result<f64> {
        let since = now_secs() - 86400;
        let result = sqlx::query_scalar::<_, f64>(
            "SELECT COALESCE(SUM(cost_usd), 0.0) FROM metrics WHERE timestamp >= ?1",
        )
        .bind(since)
        .fetch_one(&self.pool)
        .await
        .context("Querying daily cost")?;
        Ok(result)
    }

    /// Rolling monthly cost (last 30 days) in USD.
    pub async fn monthly_cost_usd(&self) -> Result<f64> {
        let since = now_secs() - 86400 * 30;
        let result = sqlx::query_scalar::<_, f64>(
            "SELECT COALESCE(SUM(cost_usd), 0.0) FROM metrics WHERE timestamp >= ?1",
        )
        .bind(since)
        .fetch_one(&self.pool)
        .await
        .context("Querying monthly cost")?;
        Ok(result)
    }

    /// List hourly rollups for a time window (for graphing).
    pub async fn rollups(&self, since: i64, until: i64) -> Result<Vec<MetricRollup>> {
        let rows = sqlx::query_as!(
            MetricRollup,
            r"SELECT id, session_id, hour_bucket, tokens_in, tokens_out, tool_calls, cost_usd
              FROM metric_rollups WHERE hour_bucket >= ?1 AND hour_bucket <= ?2
              ORDER BY hour_bucket ASC",
            since,
            until,
        )
        .fetch_all(&self.pool)
        .await
        .context("Listing metric rollups")?;
        Ok(rows)
    }
}

fn now_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn make_store() -> MetricsStore {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let store = MetricsStore::new(pool);
        store.migrate().await.unwrap();
        store
    }

    #[tokio::test]
    async fn test_record_and_summary() {
        let store = make_store().await;
        store.record("sess1", 100, 200, 3, 0.005).await.unwrap();
        store.record("sess1", 50, 100, 1, 0.002).await.unwrap();
        store.record("sess2", 200, 400, 5, 0.010).await.unwrap();

        let summary = store.summary(0, now_secs() + 1).await.unwrap();
        assert_eq!(summary.total_tokens_in, 350);
        assert_eq!(summary.total_tokens_out, 700);
        assert_eq!(summary.total_tool_calls, 9);
        assert_eq!(summary.session_count, 2);
    }

    #[tokio::test]
    async fn test_daily_cost() {
        let store = make_store().await;
        store.record("sess1", 1000, 2000, 10, 0.050).await.unwrap();
        let cost = store.daily_cost_usd().await.unwrap();
        assert!(cost > 0.0);
    }
}
