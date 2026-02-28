-- Migration 051: audit_log — immutable admin action log (Sprint UU — SOC2 CC6/CC7)
-- Retained 2 years. Write-only from API; no UPDATE or DELETE allowed at application layer.

CREATE TABLE IF NOT EXISTS audit_log (
    id          TEXT PRIMARY KEY NOT NULL DEFAULT (lower(hex(randomblob(16)))),
    actor_id    TEXT NOT NULL,          -- user_id or "daemon" or "system"
    action      TEXT NOT NULL,          -- e.g. "user.delete", "session.force_stop", "pack.yank"
    resource_type TEXT,                 -- e.g. "user", "session", "pack"
    resource_id   TEXT,                 -- UUID or slug of the affected resource
    ip_address    TEXT,                 -- IPv4 or IPv6 of the request origin
    metadata_json TEXT,                 -- optional structured context (sanitized, no secrets)
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Index for common queries: by actor, by action, by resource, by time window
CREATE INDEX IF NOT EXISTS idx_audit_log_actor     ON audit_log (actor_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_action    ON audit_log (action);
CREATE INDEX IF NOT EXISTS idx_audit_log_resource  ON audit_log (resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_created   ON audit_log (created_at);
