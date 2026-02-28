-- 050_pack_ratings.sql — Pack user ratings (Sprint TT SK.1).
--
-- Users can rate installed packs 1–5 stars.
-- Average is computed at query time (AVG(rating)).

CREATE TABLE IF NOT EXISTS pack_ratings (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(8)))),
    pack_slug   TEXT NOT NULL,
    user_id     TEXT NOT NULL,
    rating      INTEGER NOT NULL CHECK (rating BETWEEN 1 AND 5),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE (pack_slug, user_id)
);

CREATE INDEX IF NOT EXISTS idx_pack_ratings_slug ON pack_ratings(pack_slug);
