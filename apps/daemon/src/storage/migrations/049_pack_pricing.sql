-- 049_pack_pricing.sql — Pack marketplace catalog + pricing (Sprint SS MP.1).
--
-- Creates the packs table: registry of all marketplace packs (different from
-- installed_packs which tracks locally installed ones).
-- Adds pack_install_tokens for paid pack license enforcement.
-- Adds pack_purchases for purchase record-keeping.

CREATE TABLE IF NOT EXISTS packs (
    id                          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(8)))),
    slug                        TEXT NOT NULL UNIQUE,
    name                        TEXT NOT NULL,
    description                 TEXT,
    publisher                   TEXT,
    version                     TEXT,
    price_usd                   REAL,
    price_type                  TEXT CHECK (price_type IN ('free', 'one_time', 'monthly')) DEFAULT 'free',
    stripe_product_id           TEXT,
    stripe_price_id             TEXT,
    author_stripe_account_id    TEXT,
    created_at                  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at                  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE IF NOT EXISTS pack_install_tokens (
    id          TEXT PRIMARY KEY,
    pack_slug   TEXT NOT NULL,
    user_id     TEXT NOT NULL,
    token       TEXT NOT NULL UNIQUE,
    issued_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    expires_at  TEXT NOT NULL,
    revoked     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_pack_tokens_slug ON pack_install_tokens(pack_slug, user_id);

CREATE TABLE IF NOT EXISTS pack_purchases (
    id                  TEXT PRIMARY KEY,
    pack_slug           TEXT NOT NULL,
    user_id             TEXT NOT NULL,
    stripe_session_id   TEXT UNIQUE,
    price_usd           REAL NOT NULL,
    platform_fee_usd    REAL NOT NULL,
    author_amount_usd   REAL NOT NULL,
    status              TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed', 'refunded')),
    purchased_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_pack_purchases_user ON pack_purchases(user_id, purchased_at);
CREATE INDEX IF NOT EXISTS idx_pack_purchases_slug ON pack_purchases(pack_slug, purchased_at);
