-- Token usage per AI response.
-- Stores input/output token counts and estimated USD cost for every message
-- that returns usage data from the provider.
CREATE TABLE IF NOT EXISTS token_usage (
    id                  TEXT    NOT NULL PRIMARY KEY,
    session_id          TEXT    NOT NULL,
    message_id          TEXT,   -- NULL when recorded outside a message context
    model_id            TEXT    NOT NULL,
    input_tokens        INTEGER NOT NULL DEFAULT 0,
    output_tokens       INTEGER NOT NULL DEFAULT 0,
    estimated_cost_usd  REAL    NOT NULL DEFAULT 0.0,
    recorded_at         TEXT    NOT NULL  -- RFC 3339
);

CREATE INDEX IF NOT EXISTS idx_token_usage_session    ON token_usage (session_id);
CREATE INDEX IF NOT EXISTS idx_token_usage_recorded   ON token_usage (recorded_at);
