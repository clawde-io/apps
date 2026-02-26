-- Sprint CC IE.1 — Session Intent Capture
-- Stores parsed intent from the first user message + execution summary at session end.

ALTER TABLE sessions ADD COLUMN intent_json    TEXT;
ALTER TABLE sessions ADD COLUMN execution_json TEXT;
ALTER TABLE sessions ADD COLUMN intent_divergence_score REAL;
