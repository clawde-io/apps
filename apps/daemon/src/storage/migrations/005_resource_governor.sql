-- Phase 44: Resource Governor & Session Memory Architecture
-- Adds session tiering, context management, and resource metrics tables.

-- ─── Session tier fields ──────────────────────────────────────────────────────
-- These are added as idempotent ALTERs in the migrate() function, not here,
-- because SQLite doesn't support ALTER TABLE IF NOT EXISTS.

-- ─── Context snapshots ────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS context_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    snapshot_type TEXT NOT NULL CHECK (snapshot_type IN ('summary', 'task_state', 'full')),
    content TEXT NOT NULL,
    token_estimate INTEGER NOT NULL DEFAULT 0,
    message_range_start TEXT,
    message_range_end TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- ─── Tool results (full content, potentially large) ──────────────────────────
CREATE TABLE IF NOT EXISTS tool_results_full (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    full_result TEXT NOT NULL,
    truncated_preview TEXT,
    byte_size INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);

-- ─── Resource metrics (24h retention, polled every 5s) ───────────────────────
CREATE TABLE IF NOT EXISTS resource_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL DEFAULT (unixepoch()),
    total_ram_bytes INTEGER NOT NULL DEFAULT 0,
    used_ram_bytes INTEGER NOT NULL DEFAULT 0,
    daemon_ram_bytes INTEGER NOT NULL DEFAULT 0,
    active_session_count INTEGER NOT NULL DEFAULT 0,
    warm_session_count INTEGER NOT NULL DEFAULT 0,
    cold_session_count INTEGER NOT NULL DEFAULT 0,
    pool_worker_count INTEGER NOT NULL DEFAULT 0,
    context_compressions INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_resource_metrics_ts ON resource_metrics(timestamp DESC)
