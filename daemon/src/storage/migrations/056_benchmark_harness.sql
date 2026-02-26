-- Migration 056: benchmark_tasks + benchmark_runs (Sprint ZZ EH.T01/T02)
-- Continuous evaluation harness — measure AI task completion quality over time.

CREATE TABLE IF NOT EXISTS benchmark_tasks (
    id                  TEXT PRIMARY KEY NOT NULL,
    description         TEXT NOT NULL,
    initial_state_ref   TEXT,                   -- git commit or fixture identifier
    task_prompt         TEXT NOT NULL,
    success_criteria_json TEXT NOT NULL DEFAULT '[]',  -- [{type, pattern, file}]
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE IF NOT EXISTS benchmark_runs (
    id                  TEXT PRIMARY KEY NOT NULL DEFAULT (lower(hex(randomblob(16)))),
    task_id             TEXT NOT NULL,
    provider            TEXT NOT NULL,
    git_ref             TEXT,                   -- HEAD SHA at run time
    started_at          TEXT NOT NULL,
    duration_ms         INTEGER,
    turns               INTEGER,
    diff_lines          INTEGER,
    success             INTEGER NOT NULL DEFAULT 0,   -- 0 or 1 (SQLite bool)
    criteria_results_json TEXT,                 -- [{criterion, passed, detail}]
    instruction_hash    TEXT,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),

    FOREIGN KEY (task_id) REFERENCES benchmark_tasks(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_benchmark_runs_task     ON benchmark_runs (task_id);
CREATE INDEX IF NOT EXISTS idx_benchmark_runs_provider ON benchmark_runs (provider);
CREATE INDEX IF NOT EXISTS idx_benchmark_runs_git_ref  ON benchmark_runs (git_ref);

-- Content labels for injection defense (Sprint ZZ PI.T01)
CREATE TABLE IF NOT EXISTS content_labels (
    id              TEXT PRIMARY KEY NOT NULL DEFAULT (lower(hex(randomblob(16)))),
    session_id      TEXT NOT NULL,
    source_type     TEXT NOT NULL,  -- 'file','git_log','git_diff','stderr','web_fetch','mcp_tool_response','user_input'
    risk_level      TEXT NOT NULL DEFAULT 'low' CHECK (risk_level IN ('low','medium','high')),
    flagged_patterns_json TEXT,     -- JSON array of matched patterns
    content_preview TEXT,           -- first 200 chars (sanitized)
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_content_labels_session ON content_labels (session_id);
CREATE INDEX IF NOT EXISTS idx_content_labels_risk    ON content_labels (risk_level);
