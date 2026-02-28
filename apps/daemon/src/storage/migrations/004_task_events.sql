-- Phase 43b: Task State Engine
-- Event-sourced audit log for all task state transitions.
-- Complements the JSONL event log on disk and provides a queryable index.

CREATE TABLE IF NOT EXISTS task_events (
    task_id        TEXT    NOT NULL,
    event_seq      INTEGER NOT NULL,
    event_type     TEXT    NOT NULL,
    actor          TEXT    NOT NULL,
    correlation_id TEXT    NOT NULL,
    data_json      TEXT    NOT NULL,
    ts             TEXT    NOT NULL,
    PRIMARY KEY (task_id, event_seq)
);

CREATE INDEX IF NOT EXISTS idx_task_events_ts       ON task_events(ts DESC);
CREATE INDEX IF NOT EXISTS idx_task_events_task     ON task_events(task_id, event_seq);
CREATE INDEX IF NOT EXISTS idx_task_events_type     ON task_events(event_type, ts DESC);
CREATE INDEX IF NOT EXISTS idx_task_events_actor    ON task_events(actor, ts DESC);
