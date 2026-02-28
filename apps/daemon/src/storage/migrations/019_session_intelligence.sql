-- Migration 019: Session Intelligence (Sprint G, SI.T01 + SI.T04 + SI.T06)
--
-- 1. messages.token_count  — heuristic token count for each message (SI.T01)
-- 2. messages.pinned       — pinned messages always included in context (SI.T04)
-- 3. session_health        — response quality signals per session (SI.T06)

ALTER TABLE messages ADD COLUMN token_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE messages ADD COLUMN pinned       INTEGER NOT NULL DEFAULT 0;

-- Session health: tracks response quality signals per session.
-- health_score is computed from consecutive_short_responses, error_rate,
-- and truncation_count.  Updated after every AI response.
CREATE TABLE IF NOT EXISTS session_health (
    session_id              TEXT PRIMARY KEY,
    health_score            INTEGER NOT NULL DEFAULT 100,  -- 0–100
    total_turns             INTEGER NOT NULL DEFAULT 0,
    consecutive_low_quality INTEGER NOT NULL DEFAULT 0,    -- resets on good response
    short_response_count    INTEGER NOT NULL DEFAULT 0,    -- responses < 50 chars
    tool_error_count        INTEGER NOT NULL DEFAULT 0,    -- tool call errors
    truncation_count        INTEGER NOT NULL DEFAULT 0,    -- detected truncations
    last_updated_at         TEXT NOT NULL DEFAULT (datetime('now'))
);
