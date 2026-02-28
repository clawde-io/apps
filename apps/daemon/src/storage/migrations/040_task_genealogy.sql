-- Sprint CC TG.1 — Task Genealogy
-- Tracks parent/child relationships between tasks, allowing full ancestry trees.

CREATE TABLE IF NOT EXISTS task_genealogy (
    id              TEXT    PRIMARY KEY DEFAULT (lower(hex(randomblob(8)))),
    parent_task_id  TEXT    NOT NULL REFERENCES agent_tasks(id) ON DELETE CASCADE,
    child_task_id   TEXT    NOT NULL REFERENCES agent_tasks(id) ON DELETE CASCADE,
    -- relationship: spawned_from | blocked_by | related_to
    relationship    TEXT    NOT NULL DEFAULT 'spawned_from',
    created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE (parent_task_id, child_task_id)
);

CREATE INDEX IF NOT EXISTS idx_task_genealogy_parent ON task_genealogy(parent_task_id);
CREATE INDEX IF NOT EXISTS idx_task_genealogy_child  ON task_genealogy(child_task_id);
