-- Repo intelligence profile storage (Sprint F — Repo Intelligence).
--
-- repo_profiles: persists the result of repo.scan (StackProfile + conventions).
-- validator_runs: stores the output of validators.run (auto-derived linters/tests).
CREATE TABLE IF NOT EXISTS repo_profiles (
    repo_path      TEXT PRIMARY KEY,
    primary_lang   TEXT NOT NULL DEFAULT 'unknown',
    secondary_langs TEXT NOT NULL DEFAULT '[]',   -- JSON array of language strings
    frameworks     TEXT NOT NULL DEFAULT '[]',    -- JSON array of framework strings
    build_tools    TEXT NOT NULL DEFAULT '[]',    -- JSON array of build tool strings
    conventions    TEXT NOT NULL DEFAULT '{}',    -- JSON object: CodeConventions
    monorepo       INTEGER NOT NULL DEFAULT 0,
    confidence     REAL NOT NULL DEFAULT 0.0,
    scanned_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS validator_runs (
    id             TEXT PRIMARY KEY,
    repo_path      TEXT NOT NULL,
    validator_cmd  TEXT NOT NULL,    -- e.g. "cargo clippy", "tsc --noEmit"
    exit_code      INTEGER,          -- NULL while still running
    output         TEXT,             -- combined stdout + stderr
    started_at     TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at    TEXT
);

CREATE INDEX IF NOT EXISTS idx_repo_profiles_updated ON repo_profiles(updated_at);
CREATE INDEX IF NOT EXISTS idx_validator_runs_repo   ON validator_runs(repo_path);
CREATE INDEX IF NOT EXISTS idx_validator_runs_start  ON validator_runs(started_at);
