-- Sprint DD PP.2 — Semantic Change Events table
CREATE TABLE IF NOT EXISTS semantic_events (
    id             TEXT PRIMARY KEY,
    session_id     TEXT,  -- linked session (NULL for batch analysis)
    task_id        TEXT,  -- linked task (NULL if no task)
    event_type     TEXT NOT NULL,  -- feature_added|bug_fixed|refactored|test_added|config_changed|dependency_updated
    affected_files TEXT NOT NULL DEFAULT '[]',  -- JSON array of file paths
    summary_text   TEXT NOT NULL DEFAULT '',
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_semantic_session ON semantic_events(session_id);
CREATE INDEX IF NOT EXISTS idx_semantic_type ON semantic_events(event_type);
CREATE INDEX IF NOT EXISTS idx_semantic_created ON semantic_events(created_at);
