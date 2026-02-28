-- Migration 038: enterprise_policies table — Sprint Z, EN.T01
--
-- Stores org-level policy rules that the daemon enforces on every session.
-- Policy types:
--   'provider_allowlist'  — only the listed providers may be used
--   'model_allowlist'     — only the listed model names may be used
--   'path_restriction'    — sessions may not access paths outside the allowlist
--   'approval_required'   — tool calls matching the rule must be human-approved

CREATE TABLE IF NOT EXISTS enterprise_policies (
    id          TEXT    PRIMARY KEY,
    org_id      TEXT    NOT NULL,
    policy_type TEXT    NOT NULL
                        CHECK (policy_type IN (
                            'provider_allowlist',
                            'model_allowlist',
                            'path_restriction',
                            'approval_required'
                        )),
    -- JSON object whose shape depends on policy_type:
    --   provider_allowlist  : { "providers": ["claude", "codex"] }
    --   model_allowlist     : { "models": ["claude-opus-4", "gpt-4o"] }
    --   path_restriction    : { "allowed_paths": ["/workspace/…"], "deny_outside": true }
    --   approval_required   : { "tools": ["bash", "file_write"], "risk_threshold": "medium" }
    config_json TEXT    NOT NULL DEFAULT '{}',
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_enterprise_policies_org
    ON enterprise_policies(org_id);

CREATE INDEX IF NOT EXISTS idx_enterprise_policies_org_type
    ON enterprise_policies(org_id, policy_type);
