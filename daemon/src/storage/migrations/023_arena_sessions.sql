-- Migration 023: Arena Mode — blind model comparison sessions and leaderboard
--
-- arena_sessions: pairs two parallel sessions for blind provider comparison.
--   session_a_id / session_b_id: the two live sessions running in parallel.
--   provider_a / provider_b: provider names for each session (shown after vote).
--   prompt: the original prompt sent to both sessions.
--
-- arena_votes: one vote per arena session, recording which provider won.
--   winner_provider: the name of the winning provider.
--   task_type: user-supplied task category for leaderboard segmentation.
--   Valid task_type values: general | debug | refactor | explain | generate

CREATE TABLE IF NOT EXISTS arena_sessions (
    id           TEXT NOT NULL PRIMARY KEY,
    session_a_id TEXT NOT NULL,
    session_b_id TEXT NOT NULL,
    provider_a   TEXT NOT NULL,
    provider_b   TEXT NOT NULL,
    prompt       TEXT NOT NULL,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS arena_votes (
    id              TEXT NOT NULL PRIMARY KEY,
    arena_id        TEXT NOT NULL REFERENCES arena_sessions(id) ON DELETE CASCADE,
    winner_provider TEXT NOT NULL,
    task_type       TEXT NOT NULL DEFAULT 'general',
    voted_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_arena_votes_arena
    ON arena_votes(arena_id);

CREATE INDEX IF NOT EXISTS idx_arena_votes_provider_type
    ON arena_votes(winner_provider, task_type);
