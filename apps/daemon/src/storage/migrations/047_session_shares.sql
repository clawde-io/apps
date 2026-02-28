-- Sprint EE CS.1 — session_shares table.
-- Stores share tokens for live session sharing (Cloud tier).

CREATE TABLE IF NOT EXISTS session_shares (
    id          TEXT NOT NULL PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    share_token TEXT NOT NULL UNIQUE,
    team_id     TEXT,
    allow_send  INTEGER NOT NULL DEFAULT 0,  -- 0 = read-only, 1 = co-pilot
    revoked_at  TEXT,
    expires_at  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_session_shares_session
    ON session_shares(session_id);

CREATE INDEX IF NOT EXISTS idx_session_shares_token
    ON session_shares(share_token);
