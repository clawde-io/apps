-- Migration 055: evidence_packs — immutable task completion evidence (Sprint ZZ EP.T01)
-- Every completed task must have an evidence pack before being marked done.

CREATE TABLE IF NOT EXISTS evidence_packs (
    id                  TEXT PRIMARY KEY NOT NULL DEFAULT (lower(hex(randomblob(16)))),
    task_id             TEXT NOT NULL,
    run_id              TEXT,                   -- optional: session or run identifier
    instruction_hash    TEXT,                   -- SHA-256 of compiled CLAUDE.md at session start
    policy_hash         TEXT,                   -- SHA-256 of policy engine config
    worktree_commit     TEXT,                   -- git commit SHA of worktree at completion
    diff_stats_json     TEXT,                   -- { files_changed, insertions, deletions }
    test_results_json   TEXT,                   -- { passed, failed, duration_ms }
    tool_trace_json     TEXT,                   -- { reads, writes, executes, denied }
    reviewer_verdict    TEXT,                   -- 'approved', 'approved_with_comments', 'changes_requested'
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),

    FOREIGN KEY (task_id) REFERENCES agent_tasks(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_evidence_packs_task ON evidence_packs (task_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_evidence_packs_task_unique ON evidence_packs (task_id);
