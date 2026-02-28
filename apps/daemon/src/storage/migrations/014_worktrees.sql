-- Persistent worktree tracking for task-scoped git worktrees.
-- Each row tracks a worktree created by `worktrees.create` (WI.T03).
-- The in-memory WorktreeManager is backed by this table on restart.
CREATE TABLE IF NOT EXISTS worktrees (
    task_id       TEXT PRIMARY KEY,
    worktree_path TEXT NOT NULL,
    branch        TEXT NOT NULL,
    repo_path     TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'active',  -- active | done | abandoned | merged
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_worktrees_status ON worktrees(status);
