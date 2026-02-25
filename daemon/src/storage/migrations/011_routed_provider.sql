-- Migration 011: add routed_provider column to sessions table
-- Records which provider was actually selected when `provider = 'auto'` is used
-- in session.create.  NULL means the session was created with an explicit provider.
ALTER TABLE sessions ADD COLUMN routed_provider TEXT;
