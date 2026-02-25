// SPDX-License-Identifier: MIT
//! Analytics data models — serialisable types returned by the analytics RPCs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Personal Analytics ───────────────────────────────────────────────────────

/// Top-level personal usage summary returned by `analytics.personal`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalAnalytics {
    /// Total lines of code written across all AI-assisted sessions
    /// (estimated from git diff stats for sessions that touched a repo).
    pub lines_written: u64,

    /// Percentage of sessions that involved AI assistance vs. manual-only commits.
    /// Range: 0.0–100.0.
    pub ai_assist_percent: f32,

    /// Map of language name (e.g. "Rust", "Dart") to total lines attributed to it.
    pub languages: HashMap<String, u64>,

    /// Session counts grouped by calendar day (ISO 8601 date string, e.g. "2026-02-25").
    pub sessions_per_day: Vec<DailyCount>,
}

// ─── Provider Breakdown ───────────────────────────────────────────────────────

/// Per-provider usage summary returned by `analytics.providerBreakdown`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderBreakdown {
    /// Provider identifier, e.g. `"claude"`, `"codex"`, `"cursor"`.
    pub provider: String,

    /// Total number of sessions that used this provider.
    pub sessions: u64,

    /// Cumulative token count (input + output) across all sessions for this provider.
    pub tokens: u64,

    /// Estimated total cost in USD (0.0 when cost data is unavailable).
    pub cost_usd: f64,

    /// Arena win-rate for this provider (0.0–1.0). `None` if Arena has not been used yet.
    pub win_rate: Option<f32>,
}

// ─── Daily Count ──────────────────────────────────────────────────────────────

/// A (date, count) pair used in time-series data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyCount {
    /// ISO 8601 calendar date, e.g. `"2026-02-25"`.
    pub date: String,

    /// The count for this day (sessions, messages, lines, etc.).
    pub count: u64,
}

// ─── Session Analytics ────────────────────────────────────────────────────────

/// Per-session analytics returned by `analytics.session`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAnalytics {
    /// The session identifier.
    pub session_id: String,

    /// Total wall-clock time of the session in seconds
    /// (from `created_at` to `updated_at`).
    pub duration_secs: u64,

    /// Total number of messages in the session (both user and AI).
    pub message_count: u64,

    /// Provider that handled the session.
    pub provider: String,

    /// Lines written in this session (from git diff, or 0 if no repo was attached).
    pub lines_written: u64,
}

// ─── Achievement ──────────────────────────────────────────────────────────────

/// A single achievement badge in the achievement system (AN.T05–AN.T06).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    /// Machine-stable identifier, SCREAMING_SNAKE_CASE string, e.g. `"first_session"`.
    pub id: String,

    /// Human-readable badge name, e.g. `"First Session"`.
    pub name: String,

    /// Short description shown on the achievement card.
    pub description: String,

    /// Whether the achievement has been unlocked.
    pub unlocked: bool,

    /// ISO 8601 timestamp when the achievement was unlocked. `None` if not yet unlocked.
    pub unlocked_at: Option<String>,
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_count_fields() {
        let dc = DailyCount { date: "2026-02-25".to_string(), count: 42 };
        assert_eq!(dc.date, "2026-02-25");
        assert_eq!(dc.count, 42);
    }

    #[test]
    fn personal_analytics_roundtrip_json() {
        let mut langs = HashMap::new();
        langs.insert("Rust".to_string(), 1200u64);
        langs.insert("Dart".to_string(), 800u64);
        let pa = PersonalAnalytics {
            lines_written: 2000,
            ai_assist_percent: 85.5,
            languages: langs,
            sessions_per_day: vec![
                DailyCount { date: "2026-02-25".to_string(), count: 3 },
            ],
        };
        let json = serde_json::to_string(&pa).unwrap();
        let back: PersonalAnalytics = serde_json::from_str(&json).unwrap();
        assert_eq!(back.lines_written, 2000);
        assert!((back.ai_assist_percent - 85.5).abs() < 0.001);
        assert_eq!(back.languages["Rust"], 1200);
        assert_eq!(back.sessions_per_day.len(), 1);
    }

    #[test]
    fn provider_breakdown_optional_win_rate() {
        let pb_with = ProviderBreakdown {
            provider: "claude".to_string(),
            sessions: 10,
            tokens: 5000,
            cost_usd: 0.0,
            win_rate: Some(0.7),
        };
        let pb_without = ProviderBreakdown {
            provider: "codex".to_string(),
            sessions: 3,
            tokens: 1200,
            cost_usd: 0.0,
            win_rate: None,
        };
        assert!(pb_with.win_rate.is_some());
        assert!(pb_without.win_rate.is_none());
    }

    #[test]
    fn session_analytics_fields() {
        let sa = SessionAnalytics {
            session_id: "sess-abc".to_string(),
            duration_secs: 3600,
            message_count: 24,
            provider: "claude".to_string(),
            lines_written: 150,
        };
        assert_eq!(sa.duration_secs, 3600);
        assert_eq!(sa.message_count, 24);
    }

    #[test]
    fn achievement_locked_has_no_unlocked_at() {
        let a = Achievement {
            id: "first_session".to_string(),
            name: "First Session".to_string(),
            description: "Completed your first AI session.".to_string(),
            unlocked: false,
            unlocked_at: None,
        };
        assert!(!a.unlocked);
        assert!(a.unlocked_at.is_none());
    }

    #[test]
    fn achievement_unlocked_has_timestamp() {
        let a = Achievement {
            id: "power_user".to_string(),
            name: "Power User".to_string(),
            description: "100 sessions completed.".to_string(),
            unlocked: true,
            unlocked_at: Some("2026-02-25T12:00:00Z".to_string()),
        };
        assert!(a.unlocked);
        assert!(a.unlocked_at.is_some());
    }

    #[test]
    fn achievement_roundtrip_json() {
        let a = Achievement {
            id: "test".to_string(),
            name: "Test Badge".to_string(),
            description: "A test achievement.".to_string(),
            unlocked: true,
            unlocked_at: Some("2026-01-01T00:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: Achievement = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "test");
        assert!(back.unlocked);
    }
}
