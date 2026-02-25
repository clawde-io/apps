-- Sprint P: Checkpoint storage for git-backed state snapshots.
-- Each checkpoint records a git SHA so the daemon can restore the repo
-- to a known-good state via git reset --hard.

CREATE TABLE IF NOT EXISTS checkpoints (
    id          TEXT PRIMARY KEY,
    repo_path   TEXT NOT NULL,
    name        TEXT NOT NULL,
    description TEXT,
    git_sha     TEXT NOT NULL,
    auto_created INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_checkpoints_repo ON checkpoints (repo_path, created_at DESC);
