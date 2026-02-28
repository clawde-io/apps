-- Sprint CC AM.1 — Session File Attention Map
-- Tracks how many times each file is read, written, or mentioned in a session.

CREATE TABLE IF NOT EXISTS session_file_attention (
    id              TEXT    PRIMARY KEY DEFAULT (lower(hex(randomblob(8)))),
    session_id      TEXT    NOT NULL,
    file_path       TEXT    NOT NULL,
    read_count      INTEGER NOT NULL DEFAULT 0,
    write_count     INTEGER NOT NULL DEFAULT 0,
    mention_count   INTEGER NOT NULL DEFAULT 0,
    last_accessed_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE (session_id, file_path)
);

CREATE INDEX IF NOT EXISTS idx_session_file_attention_session ON session_file_attention(session_id);
CREATE INDEX IF NOT EXISTS idx_session_file_attention_file    ON session_file_attention(file_path);
