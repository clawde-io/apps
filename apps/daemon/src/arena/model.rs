// SPDX-License-Identifier: MIT
// Arena Mode data model types (Sprint K, AM.T01–AM.T03).

use serde::{Deserialize, Serialize};

/// A blind arena session pairing two AI provider sessions on the same prompt.
///
/// Both sessions receive the same prompt simultaneously.  The client shows
/// response A and response B without revealing which provider generated each
/// until the user casts a vote.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ArenaSession {
    /// Unique arena session identifier (UUID v4).
    pub id: String,
    /// Daemon session ID for provider A.
    pub session_a_id: String,
    /// Daemon session ID for provider B.
    pub session_b_id: String,
    /// Provider name for session A (e.g. "claude", "codex").
    pub provider_a: String,
    /// Provider name for session B.
    pub provider_b: String,
    /// The original prompt sent to both sessions.
    pub prompt: String,
    /// RFC 3339 creation timestamp.
    pub created_at: String,
}

/// A single vote recording which provider won an arena comparison.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ArenaVote {
    /// Unique vote identifier (UUID v4).
    pub id: String,
    /// Arena session this vote belongs to.
    pub arena_id: String,
    /// Name of the winning provider (e.g. "claude", "codex").
    pub winner_provider: String,
    /// Broad task category that helps segment the leaderboard.
    /// Valid values: general | debug | refactor | explain | generate
    pub task_type: String,
    /// RFC 3339 vote timestamp.
    pub voted_at: String,
}

/// Aggregated win-rate entry for the arena leaderboard (AM.T03).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    /// Provider name.
    pub provider: String,
    /// Task type this entry covers (or "all" for the aggregate row).
    pub task_type: String,
    /// Number of wins for this provider/task_type combination.
    pub wins: u64,
    /// Total votes in this provider/task_type bucket.
    pub total: u64,
    /// Win rate as a fraction in [0.0, 1.0].
    pub win_rate: f64,
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_session_fields_accessible() {
        let session = ArenaSession {
            id: "arena-1".to_string(),
            session_a_id: "sess-a".to_string(),
            session_b_id: "sess-b".to_string(),
            provider_a: "claude".to_string(),
            provider_b: "codex".to_string(),
            prompt: "Explain async/await".to_string(),
            created_at: "2026-02-25T00:00:00Z".to_string(),
        };
        assert_eq!(session.provider_a, "claude");
        assert_eq!(session.provider_b, "codex");
    }

    #[test]
    fn arena_vote_fields_accessible() {
        let vote = ArenaVote {
            id: "vote-1".to_string(),
            arena_id: "arena-1".to_string(),
            winner_provider: "claude".to_string(),
            task_type: "debug".to_string(),
            voted_at: "2026-02-25T00:01:00Z".to_string(),
        };
        assert_eq!(vote.winner_provider, "claude");
        assert_eq!(vote.task_type, "debug");
    }

    #[test]
    fn leaderboard_entry_win_rate() {
        let entry = LeaderboardEntry {
            provider: "claude".to_string(),
            task_type: "all".to_string(),
            wins: 7,
            total: 10,
            win_rate: 0.7,
        };
        assert!((entry.win_rate - 0.7).abs() < f64::EPSILON);
        assert_eq!(entry.wins + 3, entry.total);
    }

    #[test]
    fn arena_session_roundtrip_json() {
        let session = ArenaSession {
            id: "x".to_string(),
            session_a_id: "a".to_string(),
            session_b_id: "b".to_string(),
            provider_a: "claude".to_string(),
            provider_b: "codex".to_string(),
            prompt: "hello".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&session).unwrap();
        let back: ArenaSession = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "x");
        assert_eq!(back.provider_a, "claude");
    }

    #[test]
    fn leaderboard_entry_zero_total() {
        let entry = LeaderboardEntry {
            provider: "cursor".to_string(),
            task_type: "refactor".to_string(),
            wins: 0,
            total: 0,
            win_rate: 0.0,
        };
        assert_eq!(entry.total, 0);
        assert_eq!(entry.win_rate, 0.0);
    }
}
