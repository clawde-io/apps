-- Migration 052: retention_policies — per-org data retention config (Sprint UU DR.1)
-- Defaults: 90 days for Cloud tiers; configurable for Enterprise.

CREATE TABLE IF NOT EXISTS retention_policies (
    org_id          TEXT PRIMARY KEY NOT NULL,
    sessions_days   INTEGER NOT NULL DEFAULT 90,
    messages_days   INTEGER NOT NULL DEFAULT 90,
    telemetry_days  INTEGER NOT NULL DEFAULT 30,
    audit_log_days  INTEGER NOT NULL DEFAULT 730,  -- 2 years — fixed for SOC2
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Insert default policy for the "platform" org (applies to all Cloud users unless overridden)
INSERT OR IGNORE INTO retention_policies (org_id, sessions_days, messages_days, telemetry_days)
VALUES ('platform', 90, 90, 30);
