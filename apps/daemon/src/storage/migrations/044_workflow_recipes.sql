-- Sprint DD WR.1 — Workflow Recipes table
CREATE TABLE IF NOT EXISTS workflow_recipes (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    template_yaml TEXT NOT NULL,
    tags        TEXT NOT NULL DEFAULT '[]',    -- JSON array of tag strings
    is_builtin  INTEGER NOT NULL DEFAULT 0,
    run_count   INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS workflow_runs (
    id              TEXT PRIMARY KEY,
    recipe_id       TEXT NOT NULL REFERENCES workflow_recipes(id) ON DELETE CASCADE,
    status          TEXT NOT NULL DEFAULT 'running',  -- running|done|failed|cancelled
    current_step    INTEGER NOT NULL DEFAULT 0,
    total_steps     INTEGER NOT NULL DEFAULT 0,
    output_json     TEXT,
    started_at      TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at     TEXT
);

CREATE INDEX IF NOT EXISTS idx_workflow_runs_recipe ON workflow_runs(recipe_id);
