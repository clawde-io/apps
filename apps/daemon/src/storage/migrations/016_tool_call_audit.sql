-- DC.T43: Tool call audit log
-- Append-only security audit trail of every tool call attempt.
-- Not deleted when the parent session is deleted (no CASCADE on session_id).

CREATE TABLE IF NOT EXISTS tool_call_events (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL,
    tool_name       TEXT NOT NULL,
    sanitized_input TEXT,
    approved_by     TEXT NOT NULL,   -- 'auto', 'user', or 'rejected'
    rejection_reason TEXT,
    created_at      DATETIME NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_tce_session   ON tool_call_events(session_id);
CREATE INDEX IF NOT EXISTS idx_tce_created   ON tool_call_events(created_at);
