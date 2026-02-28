//! Account pool registry for the provider scheduler.
//!
//! Tracks all registered AI provider accounts (Claude, Codex, …), their
//! availability, rate-limit windows, and usage counters. This is distinct
//! from `account::AccountRegistry` which persists accounts in SQLite and
//! handles tier-based auto-switching. This module is an in-memory scheduling
//! layer on top of the stored accounts.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, warn};

// ── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountEntry {
    pub account_id: String,
    /// Provider identifier: `"claude"` | `"codex"`
    pub provider: String,
    /// Key name in the global vault (e.g. `CLAUDE_API_KEY_1`).
    pub vault_ref: String,
    pub is_available: bool,
    /// When set, the account is blocked until this time.
    pub blocked_until: Option<DateTime<Utc>>,
    /// Requests used in the current minute window.
    pub rpm_used: u32,
    /// Tokens used in the current minute window.
    pub tpm_used: u64,
    /// Lifetime request count.
    pub total_requests: u64,
    pub last_used: Option<DateTime<Utc>>,
}

// ── Pool ────────────────────────────────────────────────────────────────────

pub struct AccountPool {
    /// account_id -> entry
    accounts: RwLock<HashMap<String, AccountEntry>>,
}

impl AccountPool {
    pub fn new() -> Self {
        Self {
            accounts: RwLock::new(HashMap::new()),
        }
    }

    /// Register (or overwrite) an account in the pool.
    pub async fn register(&self, entry: AccountEntry) {
        debug!(account_id = %entry.account_id, provider = %entry.provider, "account registered");
        self.accounts
            .write()
            .await
            .insert(entry.account_id.clone(), entry);
    }

    /// Return the best available account for `provider`.
    ///
    /// "Best" means: available, not blocked, lowest `rpm_used` (least loaded).
    pub async fn get_available(&self, provider: &str) -> Option<AccountEntry> {
        let now = Utc::now();
        let map = self.accounts.read().await;

        let mut candidates: Vec<&AccountEntry> = map
            .values()
            .filter(|a| {
                a.provider == provider && a.is_available && a.blocked_until.is_none_or(|t| now >= t)
            })
            .collect();

        // Prefer the least-loaded account.
        candidates.sort_by_key(|a| a.rpm_used);
        candidates.first().cloned().cloned()
    }

    /// Block an account until `blocked_until`.
    pub async fn mark_blocked(&self, account_id: &str, blocked_until: DateTime<Utc>) {
        let mut map = self.accounts.write().await;
        if let Some(entry) = map.get_mut(account_id) {
            entry.blocked_until = Some(blocked_until);
            entry.is_available = false;
            warn!(account_id, until = %blocked_until, "account blocked");
        }
    }

    /// Mark an account as rate-limited for `retry_after_secs` seconds.
    pub async fn mark_rate_limited(&self, account_id: &str, retry_after_secs: u64) {
        let until = Utc::now() + chrono::Duration::seconds(retry_after_secs as i64);
        self.mark_blocked(account_id, until).await;
    }

    /// Record usage for an account (increment request + token counters).
    pub async fn record_usage(&self, account_id: &str, tokens: u64) {
        let mut map = self.accounts.write().await;
        if let Some(entry) = map.get_mut(account_id) {
            entry.rpm_used = entry.rpm_used.saturating_add(1);
            entry.tpm_used = entry.tpm_used.saturating_add(tokens);
            entry.total_requests = entry.total_requests.saturating_add(1);
            entry.last_used = Some(Utc::now());
        }
    }

    /// List all accounts (any state).
    pub async fn list(&self) -> Vec<AccountEntry> {
        self.accounts.read().await.values().cloned().collect()
    }

    /// Reset per-minute counters for all accounts. Call once per minute.
    pub async fn reset_minute_counters(&self) {
        let mut map = self.accounts.write().await;
        let now = Utc::now();
        for entry in map.values_mut() {
            entry.rpm_used = 0;
            entry.tpm_used = 0;
            // Unblock accounts whose block window has passed.
            if let Some(until) = entry.blocked_until {
                if now >= until {
                    entry.blocked_until = None;
                    entry.is_available = true;
                    debug!(account_id = %entry.account_id, "account unblocked");
                }
            }
        }
    }
}

impl Default for AccountPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe wrapper for use in `AppContext`.
pub type SharedAccountPool = Arc<AccountPool>;
