-- Migration 054: task file ownership + lease heartbeats (Sprint ZZ FO.T01 + LH.T01)

-- File ownership: glob patterns restricting which paths a task may modify
ALTER TABLE agent_tasks ADD COLUMN owned_paths_json TEXT;
-- e.g. '["src/payments/**","tests/payments/**"]'

-- Lease fields: heartbeat-based lease for parallel agent coordination
ALTER TABLE agent_tasks ADD COLUMN claimed_by_agent_id TEXT;
ALTER TABLE agent_tasks ADD COLUMN lease_expires_at INTEGER;   -- unix timestamp
ALTER TABLE agent_tasks ADD COLUMN last_heartbeat_at INTEGER;  -- unix timestamp

-- Index for lease janitor query
CREATE INDEX IF NOT EXISTS idx_agent_tasks_lease ON agent_tasks (lease_expires_at, status);

-- Instruction proposals table (Sprint ZZ IL.T05)
CREATE TABLE IF NOT EXISTS instruction_proposals (
    id                  TEXT PRIMARY KEY NOT NULL,
    suggested_scope     TEXT NOT NULL DEFAULT 'project',
    suggested_content   TEXT NOT NULL,
    confidence          REAL NOT NULL DEFAULT 0.0,
    recurrence_count    INTEGER NOT NULL DEFAULT 0,
    status              TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','accepted','dismissed')),
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Review findings table (needed by proposals engine)
CREATE TABLE IF NOT EXISTS review_findings (
    id              TEXT PRIMARY KEY NOT NULL DEFAULT (lower(hex(randomblob(16)))),
    session_id      TEXT NOT NULL,
    finding_text    TEXT NOT NULL,
    severity        TEXT DEFAULT 'info',
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_review_findings_session ON review_findings (session_id);
CREATE INDEX IF NOT EXISTS idx_review_findings_text    ON review_findings (finding_text);
