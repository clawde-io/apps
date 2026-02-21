# Session Manager

Multi-session orchestration with provider abstraction and event streaming. Run multiple AI coding sessions simultaneously across different providers.

## Overview

The session manager is ClawDE's orchestration layer. It handles spawning AI provider processes, streaming their output, managing session state, and enabling pause/resume across devices. Sessions persist through daemon restarts and can run in the background without any UI connected.

## Capabilities

| Feature | Description |
| --- | --- |
| Concurrent sessions | Run multiple AI sessions simultaneously |
| Provider abstraction | Unified interface for Claude, Codex, Cursor |
| Event streaming | Real-time events via WebSocket to all connected UIs |
| Background execution | Sessions continue even when UIs disconnect |
| Pause and resume | Pause a session, resume later on any device |
| Session isolation | Optional git worktree isolation per session |
| State persistence | Full state saved to SQLite, survives daemon restarts |
| Provider profiles | Multiple configurations per provider |

## Session Lifecycle

```
Creating → Active → Paused → Active → Completed
                 ↘ Failed
                 ↘ Cancelled
```

- **Creating** — Provider CLI is spawning, workspace is being prepared
- **Active** — AI is working, events are streaming
- **Paused** — Session is suspended, can be resumed
- **Completed** — Work finished successfully
- **Failed** — Session encountered an unrecoverable error
- **Cancelled** — User cancelled the session

## Provider Runners

Each AI provider has a dedicated runner that handles its specific CLI interface:

- **Claude Runner** — Spawns `claude` CLI, streams output, handles tool use events
- **Codex Runner** — Spawns `codex` CLI, streams output, handles sandbox events
- **Cursor Runner** — Generates `.cursor/` configuration files

All runners translate provider-specific events into ClawDE's unified event format, so UIs don't need to know which provider is running.

## Event Stream

Every session produces a stream of typed events:

- `session.started` / `session.completed` / `session.failed`
- `message.assistant` / `message.user`
- `tool.invoked` / `tool.completed`
- `file.created` / `file.modified` / `file.deleted`
- `validator.passed` / `validator.failed`

Events are stored as JSONL (one event per line) and broadcast via WebSocket to all connected clients in real-time.
