-- 048_push_tokens.sql — Device push notification token registry (Sprint RR PN.3).
--
-- Stores FCM (Android) and APNs (iOS) device tokens registered by mobile clients.
-- The relay reads this table to forward session push events to devices.

CREATE TABLE IF NOT EXISTS push_tokens (
    device_id   TEXT PRIMARY KEY,
    token       TEXT NOT NULL,
    platform    TEXT NOT NULL CHECK (platform IN ('apns', 'fcm')),
    registered_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_push_tokens_platform ON push_tokens(platform);
