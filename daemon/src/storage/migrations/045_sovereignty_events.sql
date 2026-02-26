-- Sprint DD TS.2 — AI Tool Sovereignty Events table
CREATE TABLE IF NOT EXISTS sovereignty_events (
    id                          TEXT PRIMARY KEY,
    tool_id                     TEXT NOT NULL,  -- e.g. "copilot", "cursor", "continue", "codex"
    event_type                  TEXT NOT NULL,  -- file_written|config_changed|session_started
    file_paths                  TEXT NOT NULL DEFAULT '[]',  -- JSON array
    session_active_at_detection TEXT,           -- session ID active when detected, or NULL
    detected_at                 TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_sovereignty_tool ON sovereignty_events(tool_id);
CREATE INDEX IF NOT EXISTS idx_sovereignty_detected ON sovereignty_events(detected_at);
