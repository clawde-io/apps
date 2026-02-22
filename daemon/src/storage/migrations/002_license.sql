-- License cache: stores the last successful /daemon/verify response.
-- One row only (REPLACE ON CONFLICT).
CREATE TABLE IF NOT EXISTS license_cache (
    id          INTEGER PRIMARY KEY DEFAULT 1,
    tier        TEXT NOT NULL,
    features    TEXT NOT NULL,   -- JSON: { "relay": bool, "autoSwitch": bool }
    cached_at   TEXT NOT NULL,   -- ISO-8601
    valid_until TEXT NOT NULL    -- ISO-8601: cached_at + 7 days grace
);

-- Accounts for multi-account pool (B7).
CREATE TABLE IF NOT EXISTS accounts (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    provider         TEXT NOT NULL,   -- "claude-code" | "codex" | "cursor"
    credentials_path TEXT NOT NULL,   -- path to credentials file
    priority         INTEGER NOT NULL DEFAULT 0,
    limited_until    TEXT             -- ISO-8601, NULL = not limited
);
