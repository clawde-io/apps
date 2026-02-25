-- Account event log: captures limit signals, priority changes, switches
CREATE TABLE IF NOT EXISTS account_events (
    id          TEXT PRIMARY KEY,
    account_id  TEXT NOT NULL,
    event_type  TEXT NOT NULL,  -- "limited" | "switched" | "priority_changed" | "created" | "deleted"
    metadata    TEXT,           -- JSON blob (session_id, cooldown_minutes, old_priority, etc.)
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_account_events_account_id ON account_events(account_id);
CREATE INDEX IF NOT EXISTS idx_account_events_created_at ON account_events(created_at DESC);
