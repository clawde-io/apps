# Mode System

clawd tracks a GCI mode per session. The mode tells the AI agent how to behave: whether to brainstorm freely, plan exhaustively, execute code, or just respond to questions. Changing mode changes the injection context the agent receives on every message.

---

## Modes

| Mode | What it means |
| --- | --- |
| `NORMAL` | Default. Respond to requests, do small tasks, have conversations. |
| `LEARN` | Deep dialogue — ask questions, capture context to memory. Never write code. |
| `STORM` | Free brainstorm — "yes and" everything. No evaluation, no code. All ideas captured. |
| `FORGE` | Exhaustive planning — write tasks, lock versions, gap analysis. No application code. |
| `CRUNCH` | Code execution — write code, never stop, follow tasks exactly. |

The daemon stores the mode per session in the `sessions.mode` column (migration `013_session_mode.sql`). It persists across daemon restarts.

---

## Setting mode

### Via RPC

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "session.setMode",
  "params": { "session_id": "sess-abc", "mode": "CRUNCH" }
}
```

Valid values: `NORMAL`, `LEARN`, `STORM`, `FORGE`, `CRUNCH`.

Returns `{ "session_id": "sess-abc", "mode": "CRUNCH" }`.

### Push event

When mode changes, the daemon broadcasts:

```json
{
  "method": "session.modeChanged",
  "params": { "session_id": "sess-abc", "mode": "CRUNCH" }
}
```

Connected clients (desktop and mobile apps) update their session display immediately.

---

## How mode affects context injection

When a session message is dispatched, the daemon prepends mode-specific instructions to the system context before sending to the AI provider. Each mode injects a short banner:

- **NORMAL** — no banner (standard behavior)
- **LEARN** — "LEARN MODE: Ask questions. Capture answers to memory. Never write code."
- **STORM** — "STORM MODE: Yes and. Generate ideas freely. Everything to ideas/. No evaluation."
- **FORGE** — "FORGE MODE: Plan exhaustively. Write tasks. Lock versions. No application code."
- **CRUNCH** — "CRUNCH MODE: Execute. Write code. Never stop. Follow tasks exactly."

This injection happens in the daemon — the client does not need to include mode instructions in every message.

---

## Mode in session state

`session.get` returns the current mode as part of the session object:

```json
{
  "id": "sess-abc",
  "mode": "CRUNCH",
  "status": "active",
  ...
}
```

`session.list` also returns mode for each session, so the client can display the active mode badge without a separate call.

---

## See Also

- [[Features/Session-Manager|Session Manager]] — session lifecycle, message flow
- [[Daemon-Reference|Daemon API Reference]] — full `session.*` RPC reference
- [[Home]]
