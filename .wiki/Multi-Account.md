# Multi-Account

ClawDE supports multiple AI provider accounts. This is useful when one account hits a
rate limit or usage cap — the daemon can switch automatically (on paid tiers) or prompt
you to confirm (Free tier).

## How account switching works

| Tier | Behavior when rate-limited |
| --- | --- |
| **Free** ($0) | Daemon pauses and shows a prompt asking you to confirm the switch |
| **Personal Remote** ($9.99/yr) | Daemon switches silently to the next account in your list |
| **Cloud** ($20+/month) | We manage the account pool — no configuration needed |

## Adding accounts (self-hosted)

Account management is configured via `config.toml`. The daemon reads all accounts in
order and rotates through them when a rate limit is encountered.

```toml
[accounts]
provider = "claude"

[[accounts.entries]]
name = "primary"
cli_path = "/usr/local/bin/claude"   # optional: override PATH lookup

[[accounts.entries]]
name = "backup"
cli_path = "/home/user/.nvm/versions/node/v20/bin/claude"
```

Each entry uses the auth credentials stored in the corresponding CLI installation.
Add accounts by authenticating with `claude auth login` in each CLI location.

## Rate limit handling

When the current session hits a rate limit (`error -32003 rateLimited`):

1. The daemon logs the event
2. On Free tier: the session is paused (`session.statusChanged { status: "paused" }`);
   a push event is sent to connected clients with a reason of `"rate_limited"`; the user
   must confirm account switch via the client app or RPC
3. On Personal Remote: the daemon selects the next account, restarts the provider
   subprocess, and resumes the session automatically

## Checking account status

```sh
clawd status
```

The output includes the active account name and whether the daemon is rate-limited.

Over JSON-RPC:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "daemon.status",
  "params": {}
}
```

Response includes `activeAccount` and `rateLimited` fields.

## Cloud tier

Cloud tier users don't configure accounts. The ClawDE relay provisions AI capacity from
our account pool and routes requests automatically. Rate limits are handled transparently
with no user intervention.

---

## Related

- [[Configuration]] — full `config.toml` reference including `[accounts]`
- [[Daemon-Reference|Daemon API Reference]] — `daemon.status`, error code `-32003`
- [[Providers]] — provider setup (Claude Code, Codex)
