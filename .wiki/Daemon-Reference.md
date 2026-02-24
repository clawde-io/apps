# Daemon Reference

The `clawd` daemon exposes a JSON-RPC 2.0 API over WebSocket on `localhost:4300` (default). This reference is for developers building third-party clients or integrating ClawDE into their tooling.

## Connection

**WebSocket URL:** `ws://127.0.0.1:4300`

**HTTP health check:** `GET http://127.0.0.1:4300/health`

The daemon shares port 4300 for both WebSocket (JSON-RPC) and a plain HTTP health endpoint.

### Authentication

Every connection must authenticate before calling any other method. The auth token is stored at:

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/clawd/auth_token` |
| Linux | `~/.local/share/clawd/auth_token` |
| Windows | `%APPDATA%\clawd\auth_token` |

The file is readable only by the current user (mode 0600 on Unix).

Send `daemon.auth` as the first message after connecting:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "daemon.auth",
  "params": { "token": "YOUR_AUTH_TOKEN" }
}
```

Response on success:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": { "authenticated": true }
}
```

If auth fails or you send any other method first, you receive an error with code `-32004` and the connection is closed.

### Rate limits

- Max 10 new connections per IP per minute
- Max 100 RPC requests per connection per second

### Message format

All messages are JSON-RPC 2.0 text frames (not binary).

```
Request:  {"jsonrpc":"2.0","id":<id>,"method":"<method>","params":{...}}
Response: {"jsonrpc":"2.0","id":<id>,"result":{...}}
Error:    {"jsonrpc":"2.0","id":<id>,"error":{"code":<code>,"message":"<msg>"}}
Push:     {"jsonrpc":"2.0","method":"<event>","params":{...}}   (no "id")
```

---

## Methods

### Daemon

| Method | Description |
|--------|-------------|
| `daemon.auth` | Authenticate with bearer token (must be first) |
| `daemon.ping` | Check if daemon is alive |
| `daemon.status` | Get daemon version, uptime, session counts, config |
| `daemon.checkUpdate` | Check for an available update on GitHub Releases |
| `daemon.applyUpdate` | Download and apply the latest update (daemon restarts) |

**`daemon.ping`**

```json
// Request
{"jsonrpc":"2.0","id":1,"method":"daemon.ping","params":{}}

// Response
{"jsonrpc":"2.0","id":1,"result":{"pong":true}}
```

**`daemon.status`**

```json
// Response
{
  "result": {
    "version": "0.1.0",
    "uptimeSecs": 3600,
    "activeSessions": 2,
    "port": 4300,
    "daemonId": "sha256-of-hardware-id"
  }
}
```

---

### Repo

| Method | Params | Description |
|--------|--------|-------------|
| `repo.open` | `{ "path": "/abs/path/to/repo" }` | Open a git repo; returns repo info |
| `repo.close` | `{ "path": "/abs/path/to/repo" }` | Close the repo (stop watching) |
| `repo.status` | `{ "path": "..." }` | Get working-tree status (staged, unstaged, untracked) |
| `repo.diff` | `{ "path": "...", "staged": bool }` | Get unified diff |
| `repo.fileDiff` | `{ "path": "...", "file": "relative/path" }` | Diff a single file |

**`repo.open`**

```json
// Request
{"jsonrpc":"2.0","id":2,"method":"repo.open","params":{"path":"/home/user/myproject"}}

// Response
{
  "result": {
    "path": "/home/user/myproject",
    "branch": "main",
    "remotes": ["origin"],
    "isClean": false
  }
}
```

---

### Session

Sessions are AI coding conversations. Each session runs one AI provider subprocess (`claude` or `codex`).

| Method | Params | Description |
|--------|--------|-------------|
| `session.create` | `{ "repoPath": "...", "provider": "claude", "model"?: "...", "permissions"?: {...} }` | Create a new session |
| `session.list` | `{}` | List all sessions |
| `session.get` | `{ "sessionId": "..." }` | Get a single session |
| `session.delete` | `{ "sessionId": "..." }` | Delete a session and its history |
| `session.sendMessage` | `{ "sessionId": "...", "content": "..." }` | Send a message; starts a turn |
| `session.getMessages` | `{ "sessionId": "...", "limit"?: 50, "before"?: "<msgId>" }` | Get message history |
| `session.pause` | `{ "sessionId": "..." }` | SIGSTOP the provider subprocess |
| `session.resume` | `{ "sessionId": "..." }` | SIGCONT the provider subprocess |
| `session.cancel` | `{ "sessionId": "..." }` | Kill the current turn; session stays |

**`session.create`**

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "session.create",
  "params": {
    "repoPath": "/home/user/myproject",
    "provider": "claude"
  }
}

// Response
{
  "result": {
    "id": "ses_01abc...",
    "repoPath": "/home/user/myproject",
    "provider": "claude",
    "status": "idle",
    "messageCount": 0,
    "createdAt": 1708700000
  }
}
```

**`session.sendMessage`**

Returns immediately. The response arrives via `session.messageCreated` and `session.messageUpdated` push events as the AI streams its reply.

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "session.sendMessage",
  "params": {
    "sessionId": "ses_01abc...",
    "content": "Add a README.md to this project"
  }
}

// Response (immediate — turn runs in background)
{
  "result": {
    "id": "msg_01xyz...",
    "sessionId": "ses_01abc...",
    "role": "user",
    "content": "Add a README.md to this project",
    "status": "done",
    "createdAt": 1708700010
  }
}
```

---

### Tool approval

By default, tool calls run automatically. If a session requires human approval (e.g. destructive operations), these methods respond to pending tool calls:

| Method | Params | Description |
|--------|--------|-------------|
| `tool.approve` | `{ "sessionId": "...", "toolCallId": "..." }` | Approve a pending tool call |
| `tool.reject` | `{ "sessionId": "...", "toolCallId": "..." }` | Reject a pending tool call |

---

### System (resource monitoring)

| Method | Params | Description |
|--------|--------|-------------|
| `system.resources` | `{}` | Current RAM usage and session tier counts |
| `system.resourceHistory` | `{ "limit"?: 60 }` | Historical resource metrics (last N minutes) |

**`system.resources`**

```json
{
  "result": {
    "ram": {
      "totalBytes": 17179869184,
      "usedBytes": 8589934592,
      "daemonBytes": 104857600,
      "usedPercent": 50
    },
    "sessions": {
      "active": 1,
      "warm": 0,
      "cold": 3
    }
  }
}
```

---

### Projects

Projects group one or more repositories under a named workspace.

| Method | Params | Description |
|--------|--------|-------------|
| `project.create` | `{ "name": "...", "repoPaths"?: [...] }` | Create a project |
| `project.list` | `{}` | List all projects |
| `project.get` | `{ "projectId": "..." }` | Get a single project |
| `project.update` | `{ "projectId": "...", "name"?: "..." }` | Rename a project |
| `project.delete` | `{ "projectId": "..." }` | Delete a project |
| `project.addRepo` | `{ "projectId": "...", "repoPath": "..." }` | Add a repo to a project |
| `project.removeRepo` | `{ "projectId": "...", "repoPath": "..." }` | Remove a repo from a project |

**`project.create`**

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 10,
  "method": "project.create",
  "params": { "name": "MyApp", "repoPaths": ["/home/user/myapp"] }
}

// Response
{
  "result": {
    "id": "prj_01abc...",
    "name": "MyApp",
    "repoPaths": ["/home/user/myapp"],
    "createdAt": 1708700000
  }
}
```

---

### Device Pairing

Device pairing lets a mobile or remote client connect to the daemon using a one-time PIN
or QR code. Paired devices receive a unique device token used for all subsequent connections.

| Method | Params | Description |
|--------|--------|-------------|
| `daemon.pairPin` | `{}` | Generate a 6-digit pairing PIN (expires in 5 min) |
| `device.pair` | `{ "pin": "123456" }` | Exchange PIN for a device token |
| `device.list` | `{}` | List all paired devices |
| `device.revoke` | `{ "deviceId": "..." }` | Revoke a device token |
| `device.rename` | `{ "deviceId": "...", "name": "..." }` | Rename a paired device |

#### Pairing flow

```
Host (daemon)          Mobile client
─────────────          ─────────────
daemon.pairPin()  →    Display QR code / PIN
                  ←    device.pair { pin: "123456" }
                  →    { deviceToken: "tok_..." }
                       Store token; reconnect with Bearer tok_...
```

**`daemon.pairPin`**

```json
// Response
{
  "result": {
    "pin": "482917",
    "expiresAt": 1708700300,
    "qrPayload": "clawd://connect?host=192.168.1.5&port=4300&pin=482917"
  }
}
```

**`device.pair`**

```json
// Request
{ "jsonrpc":"2.0","id":11,"method":"device.pair","params":{"pin":"482917"} }

// Response
{
  "result": {
    "deviceToken": "tok_01xyz...",
    "deviceId": "dev_01abc...",
    "expiresAt": null
  }
}
```

Device tokens do not expire by default. Revoke with `device.revoke` when a device is lost.

---

### Diagnostics

| Method | Params | Description |
| --- | --- | --- |
| `daemon.doctor` | `{}` | Run 8 health checks; returns per-check status |
| `daemon.checkProvider` | `{ "provider": "claude" }` | Check if a provider CLI is installed and authenticated |
| `daemon.setName` | `{ "name": "..." }` | Set a human-readable name for this daemon instance |

**`daemon.doctor`**

Returns one entry per check. `status` is `"ok"`, `"warn"`, or `"error"`.

```json
{
  "result": {
    "checks": [
      { "name": "data_dir",       "status": "ok",    "message": "~/.local/share/clawd exists" },
      { "name": "auth_token",     "status": "ok",    "message": "Token file readable (0600)" },
      { "name": "sqlite",         "status": "ok",    "message": "DB healthy, 9 migrations applied" },
      { "name": "port",           "status": "ok",    "message": "Listening on 127.0.0.1:4300" },
      { "name": "provider_claude","status": "ok",    "message": "claude 1.x.x — authenticated" },
      { "name": "mdns",           "status": "ok",    "message": "_clawd._tcp advertised" },
      { "name": "disk_space",     "status": "warn",  "message": "Disk 82% full" },
      { "name": "update",         "status": "ok",    "message": "v0.1.0 is the latest" }
    ],
    "overallStatus": "warn"
  }
}
```

**`daemon.checkProvider`**

```json
// Request
{ "jsonrpc":"2.0","id":12,"method":"daemon.checkProvider","params":{"provider":"claude"} }

// Response (healthy)
{
  "result": {
    "provider": "claude",
    "available": true,
    "version": "1.x.x",
    "authenticated": true
  }
}

// Response (not installed)
{
  "result": {
    "provider": "claude",
    "available": false,
    "error": "claude not found on PATH"
  }
}
```

---

## Push Events

The daemon sends push events (JSON-RPC notifications without an `id`) to all connected clients over the same WebSocket connection. No subscription step is required — events start arriving immediately after authentication.

| Event | When it fires | Params |
|-------|---------------|--------|
| `daemon.ready` | Daemon starts | `{ version, port }` |
| `daemon.updating` | Update download begins | `{ version }` |
| `session.statusChanged` | Session status changes | `{ sessionId, status }` |
| `session.messageCreated` | New assistant message starts | `{ sessionId, message }` |
| `session.messageUpdated` | Streaming message chunk | `{ sessionId, messageId, content, status }` |
| `session.toolCallCreated` | Tool call begins | `{ sessionId, toolCall }` |
| `session.toolCallUpdated` | Tool call completes/approved/rejected | `{ sessionId, toolCallId, status }` |

**Session status values:** `idle` · `running` · `paused` · `error`

**Example: receiving a streaming reply**

```
→ session.sendMessage (request)
← result: { id: "msg_user_..." }      // user message confirmed
← session.statusChanged { status: "running" }
← session.messageCreated { message: { id: "msg_asst_...", status: "streaming", content: "" } }
← session.messageUpdated { messageId: "msg_asst_...", content: "Here is your README..." }
← session.messageUpdated { ..., content: "Here is your README...\n\n# MyProject\n..." }
← session.toolCallCreated { toolCall: { name: "Write", input: { path: "README.md", ... } } }
← session.toolCallUpdated { toolCallId: "...", status: "done" }
← session.messageUpdated { messageId: "msg_asst_...", status: "done" }
← session.statusChanged { status: "idle" }
```

---

## Error Codes

Standard JSON-RPC error codes:

| Code | Meaning |
|------|---------|
| -32700 | Parse error — malformed JSON |
| -32600 | Invalid request — not JSON-RPC 2.0 |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |

ClawDE-specific error codes:

| Code | Constant | Meaning |
|------|----------|---------|
| -32001 | `sessionNotFound` | Session ID does not exist |
| -32002 | `providerNotAvailable` | Session is busy — a turn is in progress |
| -32003 | `rateLimited` | AI provider rate limit hit — retry after a delay |
| -32004 | `unauthorized` | Bad or missing auth token |
| -32005 | `repoNotFound` | Path is not a git repository or does not exist |
| -32006 | `sessionPaused` | Session is paused — call `session.resume` first |
| -32007 | `sessionLimitReached` | Max session count reached — delete a session first |

---

## Connection Lifecycle

```
1. Connect:      ws://127.0.0.1:4300
2. Authenticate: daemon.auth { token: "..." }
3. Use:          Call any method; receive push events
4. Disconnect:   Close the WebSocket
```

Auth must complete within 10 seconds or the connection is closed. The daemon does not send a challenge — the client must initiate auth immediately.

---

## Building a Client

A minimal Python client using the `websockets` library:

```python
#!/usr/bin/env python3
"""Minimal ClawDE daemon client — reads auth token and calls daemon.status."""

import asyncio
import json
import os
import pathlib
import websockets

def auth_token_path() -> pathlib.Path:
    match os.uname().sysname:
        case "Darwin":
            return pathlib.Path.home() / "Library/Application Support/clawd/auth_token"
        case _:
            return pathlib.Path.home() / ".local/share/clawd/auth_token"

async def main():
    token = auth_token_path().read_text().strip()
    uri = "ws://127.0.0.1:4300"

    async with websockets.connect(uri) as ws:
        # Step 1: authenticate
        await ws.send(json.dumps({
            "jsonrpc": "2.0", "id": 1,
            "method": "daemon.auth",
            "params": {"token": token}
        }))
        auth_resp = json.loads(await ws.recv())
        assert auth_resp["result"]["authenticated"], "Auth failed"
        print("Authenticated.")

        # Step 2: call daemon.status
        await ws.send(json.dumps({
            "jsonrpc": "2.0", "id": 2,
            "method": "daemon.status",
            "params": {}
        }))
        status = json.loads(await ws.recv())
        print(json.dumps(status["result"], indent=2))

asyncio.run(main())
```

Install the library: `pip install websockets`

Run: `python3 clawd_client.py`

Expected output:

```json
{
  "version": "0.1.0",
  "uptimeSecs": 3721,
  "activeSessions": 0,
  "port": 4300
}
```

---

## Project Methods

Projects are named workspaces containing one or more git repositories.

### project.create

Creates a new project.

**Params:** `{ name: string, root_path?: string, description?: string, org_slug?: string }`
**Returns:** `Project`

### project.list

Returns all projects.

**Params:** `{}`
**Returns:** `Project[]`

### project.get

Returns a project with its repos.

**Params:** `{ id: string }`
**Returns:** `ProjectWithRepos`
**Errors:** `-32023` PROJECT_NOT_FOUND

### project.update

**Params:** `{ id: string, name?: string, description?: string, org_slug?: string }`
**Returns:** `Project`

### project.delete

Deletes a project. Repos on disk are unaffected.

**Params:** `{ id: string }`
**Returns:** `{ deleted: true }`

### project.addRepo

Adds a git repository to a project. Validates the path is a real git repo.

**Params:** `{ project_id: string, repo_path: string }`
**Returns:** `ProjectRepo`
**Errors:** `-32005` REPO_NOT_FOUND (not a git repo), `-32024` REPO_ALREADY_IN_PROJECT

### project.removeRepo

**Params:** `{ project_id: string, repo_path: string }`
**Returns:** `{ removed: true }`

### daemon.setName

Sets the human-readable name for this daemon/host (e.g. "Mac Mini").

**Params:** `{ name: string }`
**Returns:** `{ ok: true }`

### daemon.pairPin

Generates a 6-digit pairing PIN valid for 10 minutes. Returns all info needed to build a pairing QR code.

**Params:** `{}`
**Returns:** `{ pin: string, expires_in_seconds: number, daemon_id: string, relay_url: string, host_name: string }`

---

## Device Pairing Methods

### device.pair

Exchange a PIN for a long-lived device token. Call this once during pairing.

**Params:** `{ pin: string, name: string, platform: "ios"|"android"|"macos"|"windows"|"linux"|"web" }`
**Returns:** `{ device_id: string, device_token: string, host_name: string, daemon_id: string, relay_url: string }`
**Errors:** `-32021` PAIR_PIN_INVALID, `-32022` PAIR_PIN_EXPIRED

### device.list

Returns all paired devices (device tokens are never included in the response).

**Params:** `{}`
**Returns:** `PairedDevice[]`

### device.revoke

Revokes a paired device. Its token becomes invalid immediately.

**Params:** `{ id: string }`
**Returns:** `{ revoked: true }`
**Errors:** `-32020` DEVICE_NOT_FOUND

### device.rename

**Params:** `{ id: string, name: string }`
**Returns:** `{ ok: true }`

---

## Provider Methods

### daemon.checkProvider

Checks whether an AI provider CLI is installed and authenticated.

**Params:** `{ provider: "claude"|"codex" }`
**Returns:** `{ installed: boolean, authenticated: boolean, version: string|null, path: string|null }`

---

## New Error Codes (Phase 56)

| Code | Constant | Meaning |
| --- | --- | --- |
| -32020 | DEVICE_NOT_FOUND | Paired device ID not found |
| -32021 | PAIR_PIN_INVALID | PIN is wrong or already used |
| -32022 | PAIR_PIN_EXPIRED | PIN has expired (10-min TTL) |
| -32023 | PROJECT_NOT_FOUND | Project ID not found |
| -32024 | REPO_ALREADY_IN_PROJECT | Repo already belongs to this project |

---

## New Push Events (Phase 56)

| Event | When | Data |
| --- | --- | --- |
| `project.created` | New project created | `{ project: Project }` |
| `project.updated` | Project renamed/updated | `{ project: Project }` |
| `project.deleted` | Project deleted | `{ project_id: string }` |
| `project.repoAdded` | Repo added to project | `{ project_id: string, repo: ProjectRepo }` |
| `project.repoRemoved` | Repo removed from project | `{ project_id: string, repo_path: string }` |
| `device.paired` | New device paired | `{ device_id: string, name: string, platform: string }` |
| `device.revoked` | Device revoked | `{ device_id: string }` |
| `relay.connected` | Daemon connected to relay | `{ relay_url: string }` |
| `relay.disconnected` | Daemon disconnected from relay | `{ reason: string }` |
