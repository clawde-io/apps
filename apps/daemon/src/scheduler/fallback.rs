//! Provider fallback engine.
//!
//! When the primary provider is rate-limited or unavailable, falls back to
//! alternative providers in order. Integrates with `AccountPool` and
//! `RateLimitTracker` for real-time availability decisions.

use std::sync::Arc;

use anyhow::{bail, Result};
use tracing::{debug, info, warn};

use super::accounts::{AccountEntry, AccountPool};
use super::rate_limits::RateLimitTracker;

// ── Config ───────────────────────────────────────────────────────────────────

/// Ordered list of providers to try for a request.
#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// Primary provider (e.g. `"claude"`).
    pub primary: String,
    /// Ordered list of fallback providers (e.g. `["codex"]`).
    pub alternatives: Vec<String>,
}

impl FallbackConfig {
    pub fn claude_first() -> Self {
        Self {
            primary: "claude".to_string(),
            alternatives: vec!["codex".to_string()],
        }
    }

    pub fn codex_first() -> Self {
        Self {
            primary: "codex".to_string(),
            alternatives: vec!["claude".to_string()],
        }
    }
}

// ── Engine ───────────────────────────────────────────────────────────────────

pub struct FallbackEngine {
    pub pool: Arc<AccountPool>,
    pub rate_limits: Arc<RateLimitTracker>,
}

impl FallbackEngine {
    pub fn new(pool: Arc<AccountPool>, rate_limits: Arc<RateLimitTracker>) -> Self {
        Self { pool, rate_limits }
    }

    /// Get the best available account for `config`.
    ///
    /// Tries the primary provider first, then each alternative in order.
    /// Returns an error only if no account is available from any provider.
    pub async fn get_account(&self, config: &FallbackConfig) -> Result<AccountEntry> {
        // Try primary.
        if let Some(entry) = self.try_get(&config.primary).await {
            return Ok(entry);
        }
        warn!(primary = %config.primary, "primary provider unavailable — trying alternatives");

        // Try alternatives in order.
        for alt in &config.alternatives {
            if let Some(entry) = self.try_get(alt).await {
                info!(provider = %alt, "using fallback provider");
                return Ok(entry);
            }
        }

        bail!(
            "no available account for primary provider '{}' or alternatives {:?}",
            config.primary,
            config.alternatives
        )
    }

    /// Attempt to get an available account for `provider`, honouring rate limits.
    async fn try_get(&self, provider: &str) -> Option<AccountEntry> {
        let entry = self.pool.get_available(provider).await?;
        if self.rate_limits.is_limited(&entry.account_id).await {
            debug!(
                account_id = %entry.account_id,
                provider,
                "account is rate-limited — skipping"
            );
            return None;
        }
        Some(entry)
    }

    /// Record that a request completed successfully.
    pub async fn record_completion(&self, account_id: &str, tokens: u64) {
        self.pool.record_usage(account_id, tokens).await;
        self.rate_limits.record_request(account_id, tokens).await;
    }

    /// Record that a request hit a provider rate limit.
    pub async fn record_rate_limit(&self, account_id: &str, retry_after_secs: u64) {
        self.pool
            .mark_rate_limited(account_id, retry_after_secs)
            .await;
    }
}

/// Thread-safe wrapper for use in `AppContext`.
pub type SharedFallbackEngine = Arc<FallbackEngine>;
