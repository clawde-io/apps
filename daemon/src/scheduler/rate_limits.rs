//! Per-account sliding window rate-limit tracker.
//!
//! Tracks requests-per-minute (RPM) and tokens-per-minute (TPM) for each
//! account using a sliding window algorithm. Also provides a helper to parse
//! `Retry-After` values from provider response headers.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use tokio::sync::Mutex;

// ── Sliding window ───────────────────────────────────────────────────────────

/// A sliding-window counter for rate limiting.
pub struct SlidingWindow {
    window_secs: u64,
    max_count: u64,
    events: VecDeque<DateTime<Utc>>,
}

impl SlidingWindow {
    pub fn new(window_secs: u64, max_count: u64) -> Self {
        Self {
            window_secs,
            max_count,
            events: VecDeque::new(),
        }
    }

    /// Discard events older than the window boundary.
    fn evict(&mut self, now: DateTime<Utc>) {
        let cutoff = now - Duration::seconds(self.window_secs as i64);
        while self.events.front().is_some_and(|t| *t <= cutoff) {
            self.events.pop_front();
        }
    }

    /// Record a new event at `at`.
    pub fn record_event(&mut self, at: DateTime<Utc>) {
        self.evict(at);
        self.events.push_back(at);
    }

    /// Count events within the current window.
    pub fn count_in_window(&mut self, now: DateTime<Utc>) -> u64 {
        self.evict(now);
        self.events.len() as u64
    }

    /// Returns `true` if the count in the current window has reached `max_count`.
    pub fn is_limited(&mut self, now: DateTime<Utc>) -> bool {
        self.count_in_window(now) >= self.max_count
    }

    /// Time until the oldest event in the window expires.
    ///
    /// Returns `None` if the window is not currently limited.
    pub fn time_until_reset(&mut self, now: DateTime<Utc>) -> Option<Duration> {
        if !self.is_limited(now) {
            return None;
        }
        self.events.front().map(|oldest| {
            let expiry = *oldest + Duration::seconds(self.window_secs as i64);
            expiry - now
        })
    }
}

// ── Tracker ──────────────────────────────────────────────────────────────────

/// Sensible defaults — providers typically allow 60 RPM and 100k TPM.
const DEFAULT_RPM_WINDOW_SECS: u64 = 60;
const DEFAULT_RPM_MAX: u64 = 60;
const DEFAULT_TPM_WINDOW_SECS: u64 = 60;
const DEFAULT_TPM_MAX: u64 = 100_000;

/// Per-account RPM + TPM sliding window tracker.
pub struct RateLimitTracker {
    /// account_id -> (rpm window, tpm window)
    trackers: Mutex<HashMap<String, (SlidingWindow, SlidingWindow)>>,
}

impl RateLimitTracker {
    pub fn new() -> Self {
        Self {
            trackers: Mutex::new(HashMap::new()),
        }
    }

    fn make_windows() -> (SlidingWindow, SlidingWindow) {
        (
            SlidingWindow::new(DEFAULT_RPM_WINDOW_SECS, DEFAULT_RPM_MAX),
            SlidingWindow::new(DEFAULT_TPM_WINDOW_SECS, DEFAULT_TPM_MAX),
        )
    }

    /// Record a completed request (1 RPM + `tokens` TPM).
    pub async fn record_request(&self, account_id: &str, tokens: u64) {
        let now = Utc::now();
        let mut map = self.trackers.lock().await;
        let (rpm, tpm) = map
            .entry(account_id.to_string())
            .or_insert_with(Self::make_windows);
        rpm.record_event(now);
        // Record one TPM event per token so that SlidingWindow.count_in_window()
        // correctly reflects cumulative token usage against DEFAULT_TPM_MAX.
        // Capped at DEFAULT_TPM_MAX to bound memory in pathological cases.
        let capped = tokens.min(DEFAULT_TPM_MAX);
        for _ in 0..capped {
            tpm.record_event(now);
        }
    }

    /// Returns `true` if the account is currently rate-limited (RPM or TPM).
    pub async fn is_limited(&self, account_id: &str) -> bool {
        let now = Utc::now();
        let mut map = self.trackers.lock().await;
        let (rpm, tpm) = map
            .entry(account_id.to_string())
            .or_insert_with(Self::make_windows);
        rpm.is_limited(now) || tpm.is_limited(now)
    }

    /// Returns remaining capacity as `(rpm_remaining, tpm_remaining)`.
    pub async fn remaining_capacity(&self, account_id: &str) -> (u64, u64) {
        let now = Utc::now();
        let mut map = self.trackers.lock().await;
        let (rpm, tpm) = map
            .entry(account_id.to_string())
            .or_insert_with(Self::make_windows);
        let rpm_used = rpm.count_in_window(now);
        let tpm_used = tpm.count_in_window(now);
        (
            DEFAULT_RPM_MAX.saturating_sub(rpm_used),
            DEFAULT_TPM_MAX.saturating_sub(tpm_used),
        )
    }
}

impl Default for RateLimitTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ── Retry-After parsing ──────────────────────────────────────────────────────

/// Parse a `Retry-After` header value into a `Duration`.
///
/// Accepts:
/// - An integer number of seconds (`"30"`, `"120"`)
/// - An HTTP-date string (`"Wed, 21 Oct 2025 07:28:00 GMT"`)
///
/// Returns `None` if the value cannot be parsed.
pub fn parse_retry_after(header_value: &str) -> Option<std::time::Duration> {
    let trimmed = header_value.trim();

    // Try integer seconds first (most common).
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(std::time::Duration::from_secs(secs));
    }

    // Try HTTP-date via chrono. RFC 2822 / RFC 7231 date format.
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(trimmed) {
        let now = Utc::now();
        let retry_at = dt.with_timezone(&Utc);
        if retry_at > now {
            let delta = retry_at.signed_duration_since(now);
            if let Ok(std_dur) = delta.to_std() {
                return Some(std_dur);
            }
        }
        // Already in the past — return zero delay.
        return Some(std::time::Duration::ZERO);
    }

    None
}

/// Thread-safe wrapper for use in `AppContext`.
pub type SharedRateLimitTracker = Arc<RateLimitTracker>;
