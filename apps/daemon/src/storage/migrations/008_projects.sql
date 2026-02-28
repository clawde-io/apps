-- Projects: named containers for one or more git repos
CREATE TABLE IF NOT EXISTS projects (
    id          TEXT PRIMARY KEY,           -- ULID
    name        TEXT NOT NULL,
    root_path   TEXT,                       -- optional parent directory (may not be a git repo)
    description TEXT,
    org_slug    TEXT,                       -- optional GitHub org (e.g. "clawde-io")
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    last_active_at INTEGER
);

CREATE TABLE IF NOT EXISTS project_repos (
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    repo_path   TEXT NOT NULL,
    added_at    INTEGER NOT NULL,
    last_opened_at INTEGER,
    PRIMARY KEY (project_id, repo_path)
);

-- Host identity
CREATE TABLE IF NOT EXISTS host_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT OR IGNORE INTO host_settings (key, value) VALUES ('host_name', '')
