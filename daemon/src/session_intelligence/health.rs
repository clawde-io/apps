// SPDX-License-Identifier: MIT
//! Session health monitor (SI.T06–T07).
//!
//! Tracks response quality signals per session and derives a 0–100 health
//! score.  A low score (< 40) triggers a proactive session refresh (SI.T07).
//!
//! ## Score formula
//!
//! Health starts at 100.  Each signal type degrades or restores it:
//!
//! | Signal          | Delta | Notes                                  |
//! |-----------------|-------|----------------------------------------|
//! | Short response  | −8    | Content < 50 chars                     |
//! | Tool error      | −5    | Provider-side tool call failure         |
//! | Truncation      | −15   | Response cut off mid-sentence           |
//! | Good response   | +5    | Resets `consecutive_low_quality` to 0  |
//!
//! The score is clamped to [0, 100].  Three consecutive low-quality signals
//! set the `consecutive_low_quality` counter; once it reaches `MAX_CONSECUTIVE`
//! the health score is hard-clamped to ≤ 20.

use crate::storage::Storage;
use anyhow::Result;
use sqlx::FromRow;

// ─── Constants ───────────────────────────────────────────────────────────────

/// If `consecutive_low_quality` exceeds this value the session is considered
/// unhealthy regardless of accumulated score.
const MAX_CONSECUTIVE: i64 = 4;

/// Health threshold below which SI.T07 should proactively refresh the session.
pub const REFRESH_THRESHOLD: i64 = 40;

/// Minimum response length (chars) to NOT be counted as "short".
const SHORT_RESPONSE_MIN_CHARS: usize = 50;

// ─── Signal types ─────────────────────────────────────────────────────────────

/// Observed signal about an AI response quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthSignal {
    /// Response text was very short (< 50 chars) — possible failure or refusal.
    ShortResponse,
    /// A tool call within the response returned an error.
    ToolError,
    /// The response appears to have been truncated mid-stream.
    Truncation,
    /// A normal, complete response — good quality.
    GoodResponse,
}

impl HealthSignal {
    /// Derive the signal from an assistant response string.
    ///
    /// Returns the most severe signal that applies; callers should also pass
    /// `has_tool_error` to indicate tool call failures detected externally.
    pub fn classify(response: &str, has_tool_error: bool, was_truncated: bool) -> Self {
        if was_truncated {
            return Self::Truncation;
        }
        if has_tool_error {
            return Self::ToolError;
        }
        if response.trim().len() < SHORT_RESPONSE_MIN_CHARS {
            return Self::ShortResponse;
        }
        Self::GoodResponse
    }
}

// ─── Health state ─────────────────────────────────────────────────────────────

/// Row type for reading from the `session_health` table.
#[derive(Debug, Clone, FromRow)]
struct SessionHealthRow {
    session_id: String,
    health_score: i64,
    total_turns: i64,
    consecutive_low_quality: i64,
    short_response_count: i64,
    tool_error_count: i64,
    truncation_count: i64,
}

/// Snapshot of session health loaded from the `session_health` table.
#[derive(Debug, Clone)]
pub struct SessionHealthState {
    pub session_id: String,
    /// 0–100 composite health score.
    pub health_score: i64,
    /// Total conversation turns recorded.
    pub total_turns: i64,
    /// Number of consecutive low-quality responses (resets on good response).
    pub consecutive_low_quality: i64,
    /// Cumulative short-response count.
    pub short_response_count: i64,
    /// Cumulative tool-error count.
    pub tool_error_count: i64,
    /// Cumulative truncation count.
    pub truncation_count: i64,
}

impl SessionHealthState {
    /// `true` when the health score is below the refresh threshold.
    pub fn needs_refresh(&self) -> bool {
        self.health_score < REFRESH_THRESHOLD
    }

    /// Compute a fresh health score from the current counters.
    ///
    /// Re-runs the formula each time so the value is consistent even if
    /// individual counters were updated externally.
    pub fn recompute_score(&self) -> i64 {
        let base: i64 = 100;
        let deductions =
            self.short_response_count * 8 + self.tool_error_count * 5 + self.truncation_count * 15;

        let mut score = (base - deductions).clamp(0, 100);

        // Hard-cap for sessions with many consecutive failures.
        if self.consecutive_low_quality >= MAX_CONSECUTIVE {
            score = score.min(20);
        }

        score
    }
}

impl From<SessionHealthRow> for SessionHealthState {
    fn from(r: SessionHealthRow) -> Self {
        Self {
            session_id: r.session_id,
            health_score: r.health_score,
            total_turns: r.total_turns,
            consecutive_low_quality: r.consecutive_low_quality,
            short_response_count: r.short_response_count,
            tool_error_count: r.tool_error_count,
            truncation_count: r.truncation_count,
        }
    }
}

// ─── Storage operations ──────────────────────────────────────────────────────

/// Load (or create) a `SessionHealthState` row for `session_id`.
pub async fn load_or_create(storage: &Storage, session_id: &str) -> Result<SessionHealthState> {
    let pool = storage.pool();

    // Insert a default row if it doesn't exist.
    sqlx::query("INSERT OR IGNORE INTO session_health (session_id) VALUES (?)")
        .bind(session_id)
        .execute(&pool)
        .await?;

    let row: SessionHealthRow = sqlx::query_as(
        "SELECT session_id, health_score, total_turns, consecutive_low_quality,
                short_response_count, tool_error_count, truncation_count
         FROM session_health WHERE session_id = ?",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await?;

    Ok(row.into())
}

/// Record a health signal after each AI response turn and return the updated state.
///
/// This function is the single write path for health updates.  It increments the
/// relevant counter, recomputes the score, and persists both.
pub async fn record_signal(
    storage: &Storage,
    session_id: &str,
    signal: HealthSignal,
) -> Result<SessionHealthState> {
    // Load or create the row first.
    let mut state = load_or_create(storage, session_id).await?;

    // Apply the signal.
    state.total_turns += 1;

    match signal {
        HealthSignal::ShortResponse => {
            state.short_response_count += 1;
            state.consecutive_low_quality += 1;
        }
        HealthSignal::ToolError => {
            state.tool_error_count += 1;
            state.consecutive_low_quality += 1;
        }
        HealthSignal::Truncation => {
            state.truncation_count += 1;
            state.consecutive_low_quality += 1;
        }
        HealthSignal::GoodResponse => {
            // Good response resets the consecutive counter.
            state.consecutive_low_quality = 0;
        }
    }

    state.health_score = state.recompute_score();

    let pool = storage.pool();

    // Persist updated counters.
    sqlx::query(
        "UPDATE session_health SET
             health_score            = ?,
             total_turns             = ?,
             consecutive_low_quality = ?,
             short_response_count    = ?,
             tool_error_count        = ?,
             truncation_count        = ?,
             last_updated_at         = datetime('now')
         WHERE session_id = ?",
    )
    .bind(state.health_score)
    .bind(state.total_turns)
    .bind(state.consecutive_low_quality)
    .bind(state.short_response_count)
    .bind(state.tool_error_count)
    .bind(state.truncation_count)
    .bind(&state.session_id)
    .execute(&pool)
    .await?;

    Ok(state)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(short: i64, errors: i64, trunc: i64, consec: i64) -> SessionHealthState {
        let mut s = SessionHealthState {
            session_id: "test".to_owned(),
            health_score: 100,
            total_turns: short + errors + trunc,
            consecutive_low_quality: consec,
            short_response_count: short,
            tool_error_count: errors,
            truncation_count: trunc,
        };
        s.health_score = s.recompute_score();
        s
    }

    #[test]
    fn test_perfect_health() {
        let s = make_state(0, 0, 0, 0);
        assert_eq!(s.health_score, 100);
        assert!(!s.needs_refresh());
    }

    #[test]
    fn test_short_responses_degrade() {
        let s = make_state(5, 0, 0, 0);
        // 5 * 8 = 40 deductions → score = 60
        assert_eq!(s.health_score, 60);
        assert!(!s.needs_refresh());
    }

    #[test]
    fn test_truncations_severe() {
        let s = make_state(0, 0, 4, 4);
        // 4 * 15 = 60, consecutive >= MAX_CONSECUTIVE → cap at 20
        assert!(s.health_score <= 20);
        assert!(s.needs_refresh());
    }

    #[test]
    fn test_consecutive_cap() {
        // Even if score would be 50, consecutive >= 4 caps at 20.
        let mut s = make_state(3, 0, 0, 4);
        s.health_score = s.recompute_score();
        assert!(s.health_score <= 20);
    }

    #[test]
    fn test_classify_truncated() {
        let signal = HealthSignal::classify("hello", false, true);
        assert_eq!(signal, HealthSignal::Truncation);
    }

    #[test]
    fn test_classify_short() {
        let signal = HealthSignal::classify("ok", false, false);
        assert_eq!(signal, HealthSignal::ShortResponse);
    }

    #[test]
    fn test_classify_good() {
        let long = "A".repeat(60);
        let signal = HealthSignal::classify(&long, false, false);
        assert_eq!(signal, HealthSignal::GoodResponse);
    }
}
