-- 049_pack_pricing.sql — Pack marketplace pricing fields (Sprint SS MP.1).
--
-- Adds price_usd and price_type to the packs table.
-- Adds pack_install_tokens for paid pack license enforcement.
-- Adds pack_purchases for purchase record-keeping.

ALTER TABLE packs ADD COLUMN price_usd REAL;
ALTER TABLE packs ADD COLUMN price_type TEXT CHECK (price_type IN ('free', 'one_time', 'monthly')) DEFAULT 'free';
ALTER TABLE packs ADD COLUMN stripe_product_id TEXT;
ALTER TABLE packs ADD COLUMN stripe_price_id TEXT;
ALTER TABLE packs ADD COLUMN author_stripe_account_id TEXT;

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
