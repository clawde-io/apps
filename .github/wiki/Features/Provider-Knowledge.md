# Provider Knowledge

clawd knows the capabilities and limitations of each AI provider. When a session starts, the daemon automatically injects a provider knowledge block into the system context. The agent then knows what tools the provider supports, what its context window is, and what patterns to avoid.

---

## What gets injected

On `session.create`, the daemon looks up the provider name and appends a provider profile to the session system context. Example for Claude:

```
Provider: claude (Claude Code)
Context window: 200,000 tokens
Strengths: code generation, refactoring, architecture, long-form reasoning
Limitations: rate limits on free tier; no image generation
Tool use: full tool call support (read, write, bash, search)
Model routing: auto (haiku → sonnet → opus based on task complexity)
```

This means you never need to tell the agent "remember, you're using Claude" — it already knows.

---

## Detecting installed providers

### providers.detect

Scans common install paths and `$PATH` for known provider CLIs. Returns what it found.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "providers.detect",
  "params": {}
}
```

```json
{
  "providers": [
    {
      "name": "claude",
      "display_name": "Claude Code",
      "version": "1.2.3",
      "cli_path": "/usr/local/bin/claude",
      "auth_status": "authenticated"
    },
    {
      "name": "codex",
      "display_name": "OpenAI Codex CLI",
      "version": "0.8.1",
      "cli_path": "/home/user/.local/bin/codex",
      "auth_status": "unauthenticated"
    }
  ]
}
```

`auth_status` is one of `authenticated`, `unauthenticated`, or `unknown` (CLI found but auth check failed).

### providers.list

Returns all providers that the daemon currently has knowledge for, including ones not installed locally.

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "providers.list",
  "params": {}
}
```

Returns an array of provider records with capabilities, context window, and tool support flags.

---

## Routing with `provider: "auto"`

When you create a session with `provider: "auto"`, the daemon selects the best installed provider for the request. The chosen provider is stored in the `routed_provider` column on the session (migration `011_routed_provider.sql`) and returned in `session.get`.

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "session.create",
  "params": { "repo_path": "/home/user/myapp", "provider": "auto" }
}
```

The `session.get` response includes `routed_provider: "claude"` so the client knows which provider was selected.

---

## Updating provider knowledge

Provider knowledge is bundled with the daemon binary. When clawd auto-updates, the knowledge base updates with it. There is no separate update step.

If you install a new provider CLI after the daemon is already running, call `providers.detect` to refresh the daemon's view. No restart needed.

---

## See Also

- [[Providers]] — how to install and authenticate provider CLIs
- [[Multi-Account|Multi-account]] — rotate accounts when one is rate-limited
- [[Daemon-Reference|Daemon API Reference]] — full `providers.*` and `session.*` RPC reference
- [[Home]]
