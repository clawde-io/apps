-- Migration 057: OTel telemetry spans + provider capability matrix (Sprint ZZ OT.T06, MP.T03)

-- OT.T06: SQLite span storage for `clawd observe` (lightweight, no external collector)
CREATE TABLE IF NOT EXISTS telemetry_spans (
    span_id         TEXT PRIMARY KEY,
    parent_span_id  TEXT,
    trace_id        TEXT NOT NULL,
    name            TEXT NOT NULL,
    attributes_json TEXT NOT NULL DEFAULT '{}',
    started_at_ms   INTEGER NOT NULL,
    duration_ms     INTEGER,
    status          TEXT NOT NULL DEFAULT 'running',  -- 'running' | 'ok' | 'error'
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_telemetry_spans_trace  ON telemetry_spans (trace_id);
CREATE INDEX IF NOT EXISTS idx_telemetry_spans_name   ON telemetry_spans (name);
CREATE INDEX IF NOT EXISTS idx_telemetry_spans_start  ON telemetry_spans (started_at_ms);

-- MP.T03: Provider capability matrix — registry of AI providers with capabilities.
-- Stores JSON like: {"supports_fork": true, "supports_mcp": true, "max_context_tokens": 200000}
CREATE TABLE IF NOT EXISTS providers (
    id                      TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(8)))),
    name                    TEXT NOT NULL UNIQUE,
    capability_matrix_json  TEXT,
    created_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Also add agent_tasks columns for evidence_pack_id tracking (EP.T03)
ALTER TABLE agent_tasks ADD COLUMN evidence_pack_id TEXT REFERENCES evidence_packs(id);
