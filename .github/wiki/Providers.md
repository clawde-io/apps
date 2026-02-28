# Provider Support

ClawDE routes your AI sessions through provider subprocesses running on your machine.
Each provider is a CLI tool you install separately — the daemon spawns it, streams output,
and manages the lifecycle.

## Supported providers

| Provider | CLI binary | Status | Best for |
| --- | --- | --- | --- |
| **Claude Code** | `claude` | Shipped | Code generation, refactoring, architecture |
| **Codex** | `codex` | Planned | Debugging, explanation, code review |
| **Cursor** | `cursor` | Planned | Editor-integrated workflows |
| **Aider** | `aider` | Planned | Commit-driven development |

---

## Claude Code (primary)

### Install

```sh
npm install -g @anthropic-ai/claude-code
# or
npx @anthropic-ai/claude-code --version
```

### Authenticate

```sh
claude auth login
```

Follow the browser flow to link your Anthropic account. The CLI stores the credential
in `~/.claude/` — the daemon reads from the same location.

### Test the integration

With the daemon running:

```sh
clawd doctor
```

The `checkProvider` check will confirm that `claude` is on your PATH and authenticated.

You can also test directly over JSON-RPC:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "daemon.checkProvider",
  "params": { "provider": "claude" }
}
```

Response when healthy:

```json
{
  "result": {
    "provider": "claude",
    "available": true,
    "version": "1.x.x",
    "authenticated": true
  }
}
```

### Create a session

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "session.create",
  "params": {
    "repoPath": "/home/user/myproject",
    "provider": "claude"
  }
}
```

The `provider` field must be `"claude"` (not `"claude-code"`).

---

## Codex (planned)

Codex support is planned for a future release. The daemon architecture supports multiple
provider runners — Codex will use the same session and message API as Claude Code.

---

## Provider routing

The daemon auto-routes based on task type when a project has a preferred provider configured.
Users can pin a specific provider per session by passing `"provider"` in `session.create`.

| Signal | Route |
| --- | --- |
| Code generation, refactoring | Claude Code |
| Debugging, review, explanation | Codex (planned) |
| User override | Whatever `provider` param specifies |

---

## Related

- [[Getting-Started]] — full install walkthrough
- [[Daemon-Reference|Daemon API Reference]] — `session.create`, `daemon.checkProvider`
- [[Configuration]] — `default_provider` in `config.toml`
