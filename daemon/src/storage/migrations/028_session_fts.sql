-- Sprint GG SS.1 — Full-text search index for session messages.
--
-- Creates a FTS5 virtual table that indexes all message content for
-- fast BM25-ranked full-text search across all sessions.
--
-- The `tokenize='porter unicode61'` option enables Porter stemming
-- (run → runs, running) and Unicode-aware tokenization.

CREATE VIRTUAL TABLE IF NOT EXISTS session_fts USING fts5(
    content,                          -- message text content (indexed)
    session_id   UNINDEXED,          -- opaque ID (not indexed)
    message_id   UNINDEXED,          -- opaque ID (not indexed)
    role         UNINDEXED,          -- "user" | "assistant" | "system"
    created_at   UNINDEXED,          -- ISO-8601 timestamp
    tokenize     = 'porter unicode61'
);

-- Backfill existing messages into the FTS table.
-- This runs once at migration time; the insert trigger handles future rows.
INSERT INTO session_fts(content, session_id, message_id, role, created_at)
SELECT content, session_id, id, role, created_at
FROM messages
WHERE content IS NOT NULL AND content != '';

-- Trigger: keep FTS in sync when a new message is inserted.
CREATE TRIGGER IF NOT EXISTS messages_ai_fts
AFTER INSERT ON messages
BEGIN
    INSERT INTO session_fts(content, session_id, message_id, role, created_at)
    VALUES (NEW.content, NEW.session_id, NEW.id, NEW.role, NEW.created_at);
END;

-- Trigger: remove FTS entry when a message is deleted.
CREATE TRIGGER IF NOT EXISTS messages_ad_fts
AFTER DELETE ON messages
BEGIN
    DELETE FROM session_fts WHERE message_id = OLD.id;
END;
