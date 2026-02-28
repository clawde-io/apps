-- Migration 053: instruction_nodes + instruction_compilations (Sprint ZZ IG.T01)
-- Instruction graph — hierarchical instruction node storage with scope-based priority.

CREATE TABLE IF NOT EXISTS instruction_nodes (
    id              TEXT PRIMARY KEY NOT NULL DEFAULT (lower(hex(randomblob(16)))),
    scope           TEXT NOT NULL CHECK (scope IN ('global','org','project','app','path')),
    scope_path      TEXT,                   -- for 'path' scope: the directory path
    priority        INTEGER NOT NULL DEFAULT 100, -- lower = higher priority; 1=critical, 100=normal
    owner           TEXT,                   -- user_id or 'system'
    mode_overlays   TEXT,                   -- JSON array: ['STORM','FORGE','CRUNCH','LEARN','ALL']
    content_md      TEXT NOT NULL,          -- instruction content (Markdown)
    effective_date  TEXT,                   -- ISO date when this node becomes active
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_instruction_nodes_scope     ON instruction_nodes (scope);
CREATE INDEX IF NOT EXISTS idx_instruction_nodes_scope_path ON instruction_nodes (scope_path);
CREATE INDEX IF NOT EXISTS idx_instruction_nodes_priority   ON instruction_nodes (priority);

CREATE TABLE IF NOT EXISTS instruction_compilations (
    id               TEXT PRIMARY KEY NOT NULL DEFAULT (lower(hex(randomblob(16)))),
    node_ids_json    TEXT NOT NULL,          -- JSON array of included node IDs
    target_format    TEXT NOT NULL CHECK (target_format IN ('claude','codex','all')),
    output_path      TEXT NOT NULL,          -- path written to (e.g. CLAUDE.md, AGENTS.md)
    instruction_hash TEXT NOT NULL,          -- SHA-256 of compiled output
    bytes_used       INTEGER NOT NULL DEFAULT 0,
    budget_bytes     INTEGER NOT NULL DEFAULT 8192, -- 8KB for claude, 65536 for codex
    compiled_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_instruction_compilations_target ON instruction_compilations (target_format);
CREATE INDEX IF NOT EXISTS idx_instruction_compilations_path   ON instruction_compilations (output_path);
