-- Drift scanner results storage.
-- Each row represents one detected drift item (a feature marked ✅ in FEATURES.md
-- with no corresponding source implementation found).
-- Populated by drift.scan RPC and background 24h scanner.
CREATE TABLE IF NOT EXISTS drift_items (
    id           TEXT PRIMARY KEY,
    feature      TEXT NOT NULL,
    severity     TEXT NOT NULL DEFAULT 'medium',  -- critical | high | medium | low
    kind         TEXT NOT NULL DEFAULT 'missing_source',  -- missing_source | missing_handler | doc_only | renamed
    message      TEXT NOT NULL,
    location     TEXT,         -- optional path hint (e.g. "src/ipc/handlers/foo.rs")
    project_path TEXT NOT NULL DEFAULT '',
    resolved     INTEGER NOT NULL DEFAULT 0,
    detected_at  TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at  TEXT
);

CREATE INDEX IF NOT EXISTS idx_drift_items_project ON drift_items(project_path);
CREATE INDEX IF NOT EXISTS idx_drift_items_severity ON drift_items(severity);
CREATE INDEX IF NOT EXISTS idx_drift_items_resolved ON drift_items(resolved);
