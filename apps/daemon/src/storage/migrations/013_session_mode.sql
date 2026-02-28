-- Add GCI mode tracking to sessions.
-- The `mode` column stores the current GCI mode for the session:
-- NORMAL | LEARN | STORM | FORGE | CRUNCH
-- Default is NORMAL (standard execution mode).
ALTER TABLE sessions ADD COLUMN mode TEXT NOT NULL DEFAULT 'NORMAL';
