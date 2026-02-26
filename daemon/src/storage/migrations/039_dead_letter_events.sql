-- Dead-letter queue for failed cross-repo / push events.
-- Stores events that could not be delivered after all retry attempts.

CREATE TABLE IF NOT EXISTS dead_letter_events (
    id                  TEXT    NOT NULL PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    source_session_id   TEXT,
    event_type          TEXT    NOT NULL,
    payload             TEXT    NOT NULL DEFAULT '{}',
    failure_reason      TEXT    NOT NULL DEFAULT '',
    retry_count         INTEGER NOT NULL DEFAULT 0,
    status              TEXT    NOT NULL DEFAULT 'pending',  -- pending | retrying | permanently_failed
    created_at          TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    last_attempted_at   TEXT
);

-- Fast lookup by session + event type (also enforces one queued entry per pair).
CREATE UNIQUE INDEX IF NOT EXISTS idx_dead_letter_session_type
    ON dead_letter_events (source_session_id, event_type);

-- Fast query of pending items for the retry worker.
CREATE INDEX IF NOT EXISTS idx_dead_letter_status
    ON dead_letter_events (status);
