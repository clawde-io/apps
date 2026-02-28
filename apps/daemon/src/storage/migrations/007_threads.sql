-- Phase 43f: Conversation Threading
-- Stores persistent control threads (one per project), task-scoped threads,
-- their turn history, and vendor session snapshots for session resume.

CREATE TABLE IF NOT EXISTS threads (
    thread_id        TEXT PRIMARY KEY,
    thread_type      TEXT NOT NULL CHECK(thread_type IN ('control', 'task', 'sub')),
    task_id          TEXT,                  -- NULL for control threads
    parent_thread_id TEXT,                  -- NULL for root threads
    status           TEXT NOT NULL DEFAULT 'active'
                          CHECK(status IN ('active', 'paused', 'completed', 'archived', 'error')),
    model_config     TEXT NOT NULL DEFAULT '{}',  -- JSON: {provider, model, max_tokens}
    created_at       TEXT NOT NULL,               -- RFC3339 UTC
    updated_at       TEXT NOT NULL,               -- RFC3339 UTC
    FOREIGN KEY (parent_thread_id) REFERENCES threads(thread_id)
);

CREATE INDEX IF NOT EXISTS idx_threads_task_id ON threads(task_id);
CREATE INDEX IF NOT EXISTS idx_threads_type    ON threads(thread_type);
CREATE INDEX IF NOT EXISTS idx_threads_status  ON threads(status);

CREATE TABLE IF NOT EXISTS thread_turns (
    turn_id    TEXT PRIMARY KEY,
    thread_id  TEXT NOT NULL,
    role       TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system', 'tool')),
    content    TEXT NOT NULL DEFAULT '',
    tool_calls TEXT NOT NULL DEFAULT '[]',  -- JSON array
    created_at TEXT NOT NULL,               -- RFC3339 UTC
    FOREIGN KEY (thread_id) REFERENCES threads(thread_id)
);

CREATE INDEX IF NOT EXISTS idx_turns_thread_id ON thread_turns(thread_id);

CREATE TABLE IF NOT EXISTS thread_session_snapshots (
    snapshot_id       TEXT PRIMARY KEY,
    thread_id         TEXT NOT NULL,
    vendor            TEXT NOT NULL,   -- 'claude' | 'codex'
    vendor_session_id TEXT NOT NULL,
    model_config      TEXT NOT NULL DEFAULT '{}',  -- JSON
    snapshot_at       TEXT NOT NULL,               -- RFC3339 UTC
    FOREIGN KEY (thread_id) REFERENCES threads(thread_id)
);

CREATE INDEX IF NOT EXISTS idx_snapshots_thread_vendor
    ON thread_session_snapshots(thread_id, vendor, snapshot_at DESC)
