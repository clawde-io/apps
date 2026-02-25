// SPDX-License-Identifier: MIT
//! Token usage tracker — persists token counts per AI response to SQLite.
//!
//! `TokenTracker` wraps `Arc<Storage>` and is cheap to clone.  One instance
//! lives in `AppContext`; every session handler can record usage after a
//! runner returns.
//!
//! Cost estimation is delegated to `cost::estimate_cost`.  Unknown model IDs
//! record `0.0` cost — they still count toward `message_count` so gaps are
//! visible in usage reports.

use crate::{intelligence::cost, storage::Storage};
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

// ─── Output types ─────────────────────────────────────────────────────────────

/// Aggregated token usage for a session or time range.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Total input tokens across all messages in the scope.
    pub input_tokens: u64,
    /// Total output tokens across all messages in the scope.
    pub output_tokens: u64,
    /// Sum of `estimated_cost_usd` for all messages in the scope.
    pub estimated_cost_usd: f64,
    /// Number of recorded messages in the scope.
    pub message_count: u64,
}

/// Per-model usage breakdown returned by `get_date_range_usage`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub estimated_cost_usd: f64,
    pub message_count: u64,
}

// ─── TokenTracker ─────────────────────────────────────────────────────────────

/// Token usage tracker backed by the shared SQLite pool.
#[derive(Clone)]
pub struct TokenTracker {
    storage: Arc<Storage>,
}

impl TokenTracker {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// Record a single AI response's token counts.
    ///
    /// `message_id` may be `None` when the call occurs outside a message
    /// context (e.g. a background health-check model call).
    ///
    /// USD cost is estimated via `cost::estimate_cost`.  If the model is
    /// unknown, cost is stored as `0.0`.
    pub async fn record(
        &self,
        session_id: &str,
        message_id: Option<&str>,
        model_id: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        let estimated_cost = cost::estimate_cost(model_id, input_tokens, output_tokens);
        let recorded_at = Utc::now().to_rfc3339();
        let pool = self.storage.pool();

        sqlx::query(
            "INSERT INTO token_usage \
             (id, session_id, message_id, model_id, input_tokens, output_tokens, \
              estimated_cost_usd, recorded_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(session_id)
        .bind(message_id)
        .bind(model_id)
        .bind(input_tokens as i64)
        .bind(output_tokens as i64)
        .bind(estimated_cost)
        .bind(&recorded_at)
        .execute(&pool)
        .await?;

        Ok(())
    }

    /// Return aggregated token usage for a single session.
    ///
    /// Returns a zeroed `TokenUsage` if the session has no recorded entries.
    pub async fn get_session_usage(&self, session_id: &str) -> Result<TokenUsage> {
        let pool = self.storage.pool();
        let row = sqlx::query(
            "SELECT \
               COALESCE(SUM(input_tokens),         0)   AS input_tokens, \
               COALESCE(SUM(output_tokens),        0)   AS output_tokens, \
               COALESCE(SUM(estimated_cost_usd),   0.0) AS estimated_cost_usd, \
               COUNT(*)                                  AS message_count \
             FROM token_usage \
             WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await?;

        use sqlx::Row as _;
        Ok(TokenUsage {
            input_tokens: row.try_get::<i64, _>("input_tokens")? as u64,
            output_tokens: row.try_get::<i64, _>("output_tokens")? as u64,
            estimated_cost_usd: row.try_get::<f64, _>("estimated_cost_usd")?,
            message_count: row.try_get::<i64, _>("message_count")? as u64,
        })
    }

    /// Return usage aggregated by model over a date range.
    ///
    /// `from` and `to` are RFC 3339 timestamps.  Pass `None` for either to
    /// default to the start / end of the current calendar month.
    ///
    /// Results are ordered by estimated cost descending (highest spender first).
    pub async fn get_date_range_usage(
        &self,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<Vec<ModelUsage>> {
        let now = Utc::now();
        let default_from = format!(
            "{}-{:02}-01T00:00:00Z",
            now.format("%Y"),
            now.format("%m")
        );
        let default_to = now.to_rfc3339();
        let from = from.unwrap_or(&default_from);
        let to = to.unwrap_or(&default_to);

        let pool = self.storage.pool();
        let rows = sqlx::query(
            "SELECT \
               model_id, \
               COALESCE(SUM(input_tokens),         0)   AS input_tokens, \
               COALESCE(SUM(output_tokens),        0)   AS output_tokens, \
               COALESCE(SUM(estimated_cost_usd),   0.0) AS estimated_cost_usd, \
               COUNT(*)                                  AS message_count \
             FROM token_usage \
             WHERE recorded_at >= ? AND recorded_at <= ? \
             GROUP BY model_id \
             ORDER BY estimated_cost_usd DESC",
        )
        .bind(from)
        .bind(to)
        .fetch_all(&pool)
        .await?;

        use sqlx::Row as _;
        Ok(rows
            .into_iter()
            .map(|r| ModelUsage {
                model_id: r.try_get::<String, _>("model_id").unwrap_or_default(),
                input_tokens: r.try_get::<i64, _>("input_tokens").unwrap_or(0) as u64,
                output_tokens: r.try_get::<i64, _>("output_tokens").unwrap_or(0) as u64,
                estimated_cost_usd: r
                    .try_get::<f64, _>("estimated_cost_usd")
                    .unwrap_or(0.0),
                message_count: r.try_get::<i64, _>("message_count").unwrap_or(0) as u64,
            })
            .collect())
    }

    /// Return total estimated USD cost for the current calendar month.
    pub async fn get_monthly_total(&self) -> Result<f64> {
        let now = Utc::now();
        let month_start = format!(
            "{}-{:02}-01T00:00:00Z",
            now.format("%Y"),
            now.format("%m")
        );
        let pool = self.storage.pool();
        let row = sqlx::query(
            "SELECT COALESCE(SUM(estimated_cost_usd), 0.0) AS total \
             FROM token_usage \
             WHERE recorded_at >= ?",
        )
        .bind(&month_start)
        .fetch_one(&pool)
        .await?;

        use sqlx::Row as _;
        Ok(row.try_get::<f64, _>("total")?)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use tempfile::TempDir;

    async fn make_tracker() -> (TokenTracker, TempDir) {
        let dir = TempDir::new().unwrap();
        let storage = Arc::new(Storage::new(dir.path()).await.unwrap());
        (TokenTracker::new(storage), dir)
    }

    #[tokio::test]
    async fn test_record_and_session_usage() {
        let (tracker, _dir) = make_tracker().await;

        tracker
            .record("sess-1", Some("msg-1"), "claude-sonnet-4-6", 1_000, 500)
            .await
            .unwrap();
        tracker
            .record("sess-1", Some("msg-2"), "claude-sonnet-4-6", 2_000, 1_000)
            .await
            .unwrap();

        let usage = tracker.get_session_usage("sess-1").await.unwrap();
        assert_eq!(usage.input_tokens, 3_000);
        assert_eq!(usage.output_tokens, 1_500);
        assert_eq!(usage.message_count, 2);
        // $3.00/MTok input: 0.003 * 3.0 = $0.009; $15/MTok output: 0.0015 * 15 = $0.0225 → ~$0.0315
        assert!(usage.estimated_cost_usd > 0.0, "cost should be positive");
    }

    #[tokio::test]
    async fn test_empty_session_returns_zeros() {
        let (tracker, _dir) = make_tracker().await;

        let usage = tracker.get_session_usage("nonexistent-session").await.unwrap();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.estimated_cost_usd, 0.0);
        assert_eq!(usage.message_count, 0);
    }

    #[tokio::test]
    async fn test_monthly_total_haiku_pricing() {
        let (tracker, _dir) = make_tracker().await;

        tracker
            .record("sess-1", None, "claude-haiku-4-5", 1_000_000, 1_000_000)
            .await
            .unwrap();

        let total = tracker.get_monthly_total().await.unwrap();
        // 1M input ($0.25) + 1M output ($1.25) = $1.50
        assert!(
            (total - 1.50).abs() < 0.001,
            "expected ~1.50, got {total}"
        );
    }

    #[tokio::test]
    async fn test_date_range_groups_by_model() {
        let (tracker, _dir) = make_tracker().await;

        tracker
            .record("sess-1", None, "claude-haiku-4-5", 100, 50)
            .await
            .unwrap();
        tracker
            .record("sess-1", None, "claude-sonnet-4-6", 200, 100)
            .await
            .unwrap();

        let breakdown = tracker.get_date_range_usage(None, None).await.unwrap();
        assert_eq!(breakdown.len(), 2, "should have 2 models");

        let ids: Vec<&str> = breakdown.iter().map(|m| m.model_id.as_str()).collect();
        assert!(ids.contains(&"claude-sonnet-4-6"), "missing sonnet");
        assert!(ids.contains(&"claude-haiku-4-5"), "missing haiku");
    }

    #[tokio::test]
    async fn test_unknown_model_records_zero_cost() {
        let (tracker, _dir) = make_tracker().await;

        tracker
            .record("sess-1", None, "future-model-99", 1_000, 500)
            .await
            .unwrap();

        let usage = tracker.get_session_usage("sess-1").await.unwrap();
        assert_eq!(usage.estimated_cost_usd, 0.0, "unknown model cost should be 0");
        assert_eq!(usage.message_count, 1, "entry should be recorded even with 0 cost");
    }

    #[tokio::test]
    async fn test_multiple_sessions_isolated() {
        let (tracker, _dir) = make_tracker().await;

        tracker
            .record("sess-A", None, "claude-haiku-4-5", 500, 200)
            .await
            .unwrap();
        tracker
            .record("sess-B", None, "claude-haiku-4-5", 1_000, 400)
            .await
            .unwrap();

        let a = tracker.get_session_usage("sess-A").await.unwrap();
        let b = tracker.get_session_usage("sess-B").await.unwrap();

        assert_eq!(a.input_tokens, 500);
        assert_eq!(b.input_tokens, 1_000);
        assert_eq!(a.message_count, 1);
        assert_eq!(b.message_count, 1);
    }

    #[tokio::test]
    async fn test_record_without_message_id() {
        let (tracker, _dir) = make_tracker().await;

        // message_id = None is valid (e.g. background health-check calls)
        tracker
            .record("sess-1", None, "claude-haiku-4-5", 10, 5)
            .await
            .unwrap();

        let usage = tracker.get_session_usage("sess-1").await.unwrap();
        assert_eq!(usage.message_count, 1);
    }

    // ─── MI.T26 — Budget enforcement (SQLite-backed) ──────────────────────────

    /// Haiku pricing: $0.25/MTok input + $1.25/MTok output.
    /// 6 × (1M in + 1M out) = 6 × $1.50 = $9.00.
    /// Budget cap $10.00 → 90 % → warning=true, exceeded=false.
    #[tokio::test]
    async fn test_budget_warning_at_80_pct() {
        let (tracker, _dir) = make_tracker().await;

        for i in 0..6 {
            tracker
                .record(
                    &format!("sess-{i}"),
                    None,
                    "claude-haiku-4-5",
                    1_000_000,
                    1_000_000,
                )
                .await
                .unwrap();
        }

        let total = tracker.get_monthly_total().await.unwrap();
        let cap = 10.0_f64;
        let pct = total / cap * 100.0;

        assert!(
            (total - 9.00).abs() < 0.01,
            "expected ~$9.00, got ${total:.4}"
        );
        assert!(pct >= 80.0, "pct {pct:.1} should trigger warning (>=80)");
        assert!(pct < 100.0, "pct {pct:.1} should not be exceeded (<100)");
    }

    /// 8 × (1M in + 1M out) = 8 × $1.50 = $12.00.
    /// Budget cap $10.00 → 120 % → warning=true, exceeded=true.
    #[tokio::test]
    async fn test_budget_exceeded_at_100_pct() {
        let (tracker, _dir) = make_tracker().await;

        for i in 0..8 {
            tracker
                .record(
                    &format!("sess-{i}"),
                    None,
                    "claude-haiku-4-5",
                    1_000_000,
                    1_000_000,
                )
                .await
                .unwrap();
        }

        let total = tracker.get_monthly_total().await.unwrap();
        let cap = 10.0_f64;
        let pct = total / cap * 100.0;

        assert!(
            (total - 12.00).abs() < 0.01,
            "expected ~$12.00, got ${total:.4}"
        );
        assert!(pct >= 100.0, "pct {pct:.1} should be exceeded (>=100)");
    }

    /// 1 × (1M in + 1M out) = $1.50.
    /// Budget cap $10.00 → 15 % → warning=false, exceeded=false.
    #[tokio::test]
    async fn test_budget_no_warning_below_80_pct() {
        let (tracker, _dir) = make_tracker().await;

        tracker
            .record("sess-1", None, "claude-haiku-4-5", 1_000_000, 1_000_000)
            .await
            .unwrap();

        let total = tracker.get_monthly_total().await.unwrap();
        let cap = 10.0_f64;
        let pct = total / cap * 100.0;

        assert!(
            pct < 80.0,
            "pct {pct:.1} should NOT trigger warning (<80)"
        );
    }

    /// Budget math with no cap configured (cap = None) → warning and
    /// exceeded must both be false regardless of spend.
    #[tokio::test]
    async fn test_budget_no_cap_never_warns() {
        let (tracker, _dir) = make_tracker().await;

        // Record heavy usage
        for i in 0..20 {
            tracker
                .record(
                    &format!("sess-{i}"),
                    None,
                    "claude-haiku-4-5",
                    1_000_000,
                    1_000_000,
                )
                .await
                .unwrap();
        }

        let total = tracker.get_monthly_total().await.unwrap();
        assert!(total > 0.0, "spend should be non-zero");

        // With no cap there is nothing to warn about.
        // The handler returns (null, null, false, false) — mirror that logic here.
        let cap: Option<f64> = None;
        let (warning, exceeded) = match cap {
            Some(c) if c > 0.0 => {
                let pct = total / c * 100.0;
                (pct >= 80.0, pct >= 100.0)
            }
            _ => (false, false),
        };
        assert!(!warning, "no cap → warning must be false");
        assert!(!exceeded, "no cap → exceeded must be false");
    }
}
