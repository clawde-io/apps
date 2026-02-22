//! Multi-account pool manager.
//!
//! Manages a set of AI provider accounts (Claude Code, Codex, Cursor).
//! Tracks rate-limit state and picks the best available account for a session.
//!
//! Feature gating:
//! - Free tier: manual switch prompt when limit hit (broadcasts `session.accountLimited`)
//! - Personal Remote ($9.99/yr): auto-switch silently (broadcasts `session.accountSwitched`)

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

use crate::ipc::event::EventBroadcaster;
use crate::license::LicenseInfo;
use crate::storage::{AccountRow, Storage};

/// Hint for account selection — prefer a specific provider if possible.
#[derive(Debug, Clone, Default)]
pub struct PickHint {
    pub provider: Option<String>,
}

/// The account pool registry.
#[derive(Clone)]
pub struct AccountRegistry {
    storage: Arc<Storage>,
    broadcaster: Arc<EventBroadcaster>,
}

impl AccountRegistry {
    pub fn new(storage: Arc<Storage>, broadcaster: Arc<EventBroadcaster>) -> Self {
        Self {
            storage,
            broadcaster,
        }
    }

    /// Pick the best available account for a new session.
    ///
    /// Selection order:
    /// 1. Provider matches hint (if given)
    /// 2. Not rate-limited (`limited_until` is null or in the past)
    /// 3. Lowest priority number (highest priority)
    pub async fn pick_account(&self, hint: &PickHint) -> Result<Option<AccountRow>> {
        let accounts = self.storage.list_accounts().await?;
        let now = Utc::now();

        let available: Vec<&AccountRow> = accounts
            .iter()
            .filter(|a| {
                // Skip if currently limited
                if let Some(ref until) = a.limited_until {
                    if let Ok(dt) = DateTime::parse_from_rfc3339(until) {
                        if now < dt.with_timezone(&Utc) {
                            return false;
                        }
                    }
                }
                // Apply provider hint
                if let Some(ref provider) = hint.provider {
                    return &a.provider == provider;
                }
                true
            })
            .collect();

        // Return highest priority (lowest number) or None
        Ok(available.into_iter().min_by_key(|a| a.priority).cloned())
    }

    /// Mark an account as rate-limited for `cooldown_minutes`.
    /// Broadcasts the appropriate event based on the license tier.
    pub async fn mark_limited(
        &self,
        account_id: &str,
        session_id: &str,
        cooldown_minutes: i64,
        license: &LicenseInfo,
    ) -> Result<()> {
        let until = (Utc::now() + Duration::minutes(cooldown_minutes)).to_rfc3339();
        self.storage
            .set_account_limited(account_id, Some(&until))
            .await
            .context("failed to mark account as limited")?;

        warn!(account_id, until = %until, "account rate-limited");

        if license.features.auto_switch {
            // Personal Remote+: auto-switch, silent notification
            self.broadcaster.broadcast(
                "session.accountSwitched",
                json!({
                    "sessionId": session_id,
                    "accountId": account_id,
                    "reason": "rate_limited",
                }),
            );
            info!(account_id, session_id, "auto-switched account");
        } else {
            // Free tier: requires manual user action
            self.broadcaster.broadcast(
                "session.accountLimited",
                json!({
                    "sessionId": session_id,
                    "accountId": account_id,
                    "requiresManualSwitch": true,
                    "limitedUntil": until,
                }),
            );
            info!(
                account_id,
                session_id, "account limited — user action required"
            );
        }

        Ok(())
    }

    /// Clear the rate-limit on an account (e.g. after cooldown expires).
    pub async fn clear_limit(&self, account_id: &str) -> Result<()> {
        self.storage.set_account_limited(account_id, None).await
    }

    /// Detect rate-limit signals in provider output text.
    ///
    /// Returns Some(cooldown_minutes) if a limit signal is found.
    pub fn detect_limit_signal(output: &str) -> Option<i64> {
        let lower = output.to_lowercase();

        // Claude Code patterns
        if lower.contains("rate limit") || lower.contains("too many requests") {
            return Some(60); // 1 hour default
        }
        if lower.contains("quota exceeded") || lower.contains("usage limit") {
            return Some(240); // 4 hours for quota
        }
        if lower.contains("capacity") && lower.contains("overloaded") {
            return Some(15); // 15 minutes for overload
        }

        // HTTP 429 in output
        if lower.contains("429") || lower.contains("rate_limit_error") {
            return Some(60);
        }

        None
    }
}
