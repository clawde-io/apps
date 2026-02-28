-- Phase 41: Agent Activity Dashboard & AFS Standardization
-- Adds task queue, activity log, agent registry, work sessions, and archive.

-- ─── Agent tasks (indexed task queue) ────────────────────────────────────────
CREATE TABLE IF NOT EXISTS agent_tasks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    type TEXT DEFAULT 'code'
        CHECK(type IN ('code','review','qa','planning','research','test','admin','infra')),
    phase TEXT,
    "group" TEXT,
    parent_id TEXT REFERENCES agent_tasks(id) ON DELETE SET NULL,
    severity TEXT DEFAULT 'medium'
        CHECK(severity IN ('critical','high','medium','low')),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending','in_progress','in_cr','in_qa','blocked','interrupted','done','deferred')),
    claimed_by TEXT,
    claimed_at INTEGER,
    started_at INTEGER,
    completed_at INTEGER,
    last_heartbeat INTEGER,
    file TEXT,
    files TEXT DEFAULT '[]',
    depends_on TEXT DEFAULT '[]',
    blocks TEXT DEFAULT '[]',
    tags TEXT DEFAULT '[]',
    notes TEXT,
    block_reason TEXT,
    estimated_minutes INTEGER,
    actual_minutes INTEGER,
    repo_path TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

-- ─── Agent activity log ───────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS agent_activity_log (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    ts INTEGER NOT NULL DEFAULT (unixepoch()),
    agent TEXT NOT NULL,
    task_id TEXT REFERENCES agent_tasks(id) ON DELETE SET NULL,
    phase TEXT,
    action TEXT NOT NULL,
    entry_type TEXT NOT NULL DEFAULT 'auto'
        CHECK(entry_type IN ('auto', 'note', 'system')),
    detail TEXT,
    meta TEXT,
    repo_path TEXT NOT NULL
);

-- ─── Agent registry ───────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS agent_registry (
    agent_id TEXT PRIMARY KEY,
    agent_type TEXT NOT NULL,
    session_id TEXT,
    status TEXT NOT NULL DEFAULT 'idle'
        CHECK(status IN ('idle','active','disconnected')),
    current_task_id TEXT REFERENCES agent_tasks(id) ON DELETE SET NULL,
    connected_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_seen INTEGER NOT NULL DEFAULT (unixepoch()),
    repo_path TEXT NOT NULL
);

-- ─── Work sessions ────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS work_sessions (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    started_at INTEGER NOT NULL DEFAULT (unixepoch()),
    ended_at INTEGER,
    tasks_completed INTEGER DEFAULT 0,
    tasks_created INTEGER DEFAULT 0,
    agents_active TEXT DEFAULT '[]',
    repo_path TEXT NOT NULL
);

-- ─── Archived tasks ───────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS agent_tasks_archive (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    type TEXT,
    phase TEXT,
    "group" TEXT,
    severity TEXT,
    status TEXT NOT NULL,
    claimed_by TEXT,
    claimed_at INTEGER,
    completed_at INTEGER,
    actual_minutes INTEGER,
    repo_path TEXT NOT NULL,
    archived_at INTEGER NOT NULL DEFAULT (unixepoch())
);

-- ─── Indexes ─────────────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_tasks_status_repo ON agent_tasks(status, repo_path);
CREATE INDEX IF NOT EXISTS idx_tasks_claimed ON agent_tasks(claimed_by, status);
CREATE INDEX IF NOT EXISTS idx_tasks_severity ON agent_tasks(severity, status, repo_path);
CREATE INDEX IF NOT EXISTS idx_tasks_phase ON agent_tasks(phase, repo_path);
CREATE INDEX IF NOT EXISTS idx_tasks_updated ON agent_tasks(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_tasks_parent ON agent_tasks(parent_id);

CREATE INDEX IF NOT EXISTS idx_activity_ts ON agent_activity_log(ts DESC);
CREATE INDEX IF NOT EXISTS idx_activity_task ON agent_activity_log(task_id, ts DESC);
CREATE INDEX IF NOT EXISTS idx_activity_agent ON agent_activity_log(agent, ts DESC);
CREATE INDEX IF NOT EXISTS idx_activity_repo ON agent_activity_log(repo_path, ts DESC);
CREATE INDEX IF NOT EXISTS idx_activity_action ON agent_activity_log(action, ts DESC);
CREATE INDEX IF NOT EXISTS idx_activity_phase ON agent_activity_log(phase, ts DESC);
CREATE INDEX IF NOT EXISTS idx_activity_entry_type ON agent_activity_log(entry_type, task_id, ts DESC);

CREATE INDEX IF NOT EXISTS idx_registry_repo ON agent_registry(repo_path, status);
CREATE INDEX IF NOT EXISTS idx_registry_last_seen ON agent_registry(last_seen DESC);
