-- Pairing PINs: short-lived one-time codes for device pairing
CREATE TABLE IF NOT EXISTS pair_pins (
    pin        TEXT PRIMARY KEY,      -- 6-digit numeric string "123456"
    expires_at INTEGER NOT NULL,      -- unix timestamp (10 min TTL)
    used       INTEGER NOT NULL DEFAULT 0  -- 0=unused, 1=used
);

-- Paired devices: remote clients that have been granted access
CREATE TABLE IF NOT EXISTS paired_devices (
    id           TEXT PRIMARY KEY,    -- ULID
    name         TEXT NOT NULL,       -- user-set label e.g. "My iPhone"
    platform     TEXT NOT NULL,       -- "ios", "android", "macos", "windows", "linux", "web"
    device_token TEXT NOT NULL UNIQUE, -- 32-char hex (UUID v4 without dashes), stored as-is for constant-time compare
    created_at   INTEGER NOT NULL,
    last_seen_at INTEGER,
    revoked      INTEGER NOT NULL DEFAULT 0  -- 0=active, 1=revoked
);

CREATE INDEX IF NOT EXISTS idx_paired_devices_token ON paired_devices(device_token);
