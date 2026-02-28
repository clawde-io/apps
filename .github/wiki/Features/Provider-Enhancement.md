# Provider Enhancement (Sprint BB)

ClawDE v0.2.1 upgrades the provider layer with Codex speed tiers, persistent connection state, prompt caching, and session context inheritance.

## Codex Speed Tiers

The daemon now routes Codex requests to two distinct model tiers based on the agent role:

| Speed | Model | Roles |
| --- | --- | --- |
| **Fast** | `codex-spark` | Router, Reviewer, QaExecutor |
| **Full** | `gpt-5.3-codex` | Planner, Implementer |

Fast roles get low-latency responses for classification and review. Full roles get highest-quality reasoning for planning and code generation. Claude always uses Haiku (Router) or Sonnet (all other roles) regardless of speed tier.

## Persistent Provider Sessions

A new `ProviderSessionRegistry` tracks in-memory state for each active session:

- **OpenAI Responses API chaining** — `previous_response_id` from the last Codex turn is stored and passed as `--previous-response-id` on the next turn. The server keeps conversation history; the daemon only sends the new user message each turn.
- **Idle eviction** — sessions idle longer than 5 minutes are automatically evicted to prevent memory growth.

The registry lives on `AppContext` as `provider_sessions: SharedProviderSessionRegistry`.

## Prompt Caching

### Anthropic (Claude)

Stable system prompt content is injected in two ordered blocks on session start:

1. **Coding standards** — language-specific rules detected from the repo
2. **Provider knowledge** — bundled API patterns for Hetzner, Vercel, Stripe, etc.

The `--resume <session_id>` flag on subsequent turns causes the Claude SDK to mark these blocks with `cache_control: {"type": "ephemeral"}`, so the Anthropic API caches the prefix across turns.

### Cache Key

`agents/prompt_cache.rs` provides `stable_prefix_hash()` — a SHA-256 of:

```
SHA-256(system_prompt || sorted_repo_context_paths || repo_HEAD_sha)
```

The hash is invalidated when the system prompt text changes, files are added/removed from the repo context, or a new commit lands. Use `prefix_changed(old, new)` to detect stale cache entries.

## Session Context Inheritance

`session.create` accepts an optional `inheritFrom` parameter (session ID string):

```json
{
  "method": "session.create",
  "params": {
    "provider": "claude",
    "repoPath": "/path/to/repo",
    "inheritFrom": "sess-abc123"
  }
}
```

When set, the new session receives a context primer assembled from:

- The last 3 completed AI turns from the source session (400-char preview each)
- All currently active task IDs from the task queue

This lets follow-up sessions continue where the previous one left off without re-reading full history.

## MCP Resources (PV.12)

ClawDE now advertises `resources: true` in its MCP capabilities. External MCP clients (Codex CLI, Cursor) can call:

| Method | Result |
| --- | --- |
| `resources/list` | All available resources (sessions, tasks, per-session messages) |
| `resources/read` with `uri: "clawd://sessions"` | JSON list of all sessions |
| `resources/read` with `uri: "clawd://tasks"` | JSON list of active agent tasks |
| `resources/read` with `uri: "clawd://session/{id}/messages"` | Message history for a session |
| `resources/read` with `uri: "clawd://task/{id}"` | Full task detail |
| `resources/read` with `uri: "clawd://repo/{path}"` | File content from the active repo |

Repo file reads are path-traversal protected: paths are canonicalized and must remain within the registered repo root.
