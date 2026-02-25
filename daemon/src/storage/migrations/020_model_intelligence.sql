-- Migration 020: Model intelligence — per-session model override + repo context registry
--
-- model_override: explicit model ID set by the user via session.setModel.
--   NULL = use the auto-router; non-NULL bypasses the classifier.
--
-- session_contexts: files/directories added by the user via session.addRepoContext.
--   Prioritised list — low-priority items are evicted first when context is tight.

ALTER TABLE sessions ADD COLUMN model_override TEXT;

CREATE TABLE IF NOT EXISTS session_contexts (
    id          TEXT    NOT NULL PRIMARY KEY,
    session_id  TEXT    NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    path        TEXT    NOT NULL,
    priority    INTEGER NOT NULL DEFAULT 5,  -- 1 (lowest) … 10 (highest)
    added_at    TEXT    NOT NULL,
    UNIQUE (session_id, path)
);

CREATE INDEX IF NOT EXISTS idx_session_contexts_session
    ON session_contexts(session_id, priority DESC);
