# Architecture

## Overview

ClawDE is built around a single principle: **the daemon is the source of truth, not the UI.**

```
┌─────────────────────────────────────────────────────┐
│                     User machine                    │
│                                                     │
│  ┌─────────────┐   JSON-RPC 2.0    ┌─────────────┐  │
│  │ Desktop app │◄──── WebSocket ───►│    clawd    │  │
│  │  (Flutter)  │   ws://127.0.0.1  │  (Rust/    │  │
│  └─────────────┘       :4300       │   Tokio)    │  │
│                                    │             │  │
│  ┌─────────────┐                   │  SQLite DB  │  │
│  │ Mobile app  │◄── relay (mTLS) ──│  Git2       │  │
│  │  (Flutter)  │   api.clawde.io   │  AI runners │  │
│  └─────────────┘                   └─────────────┘  │
└─────────────────────────────────────────────────────┘
```

## Components

### `clawd` — Rust daemon

The daemon runs as a background process on the user's machine. It:

- Manages **AI sessions** — creates, pauses, resumes, and closes them
- **Spawns AI providers** as subprocesses (`claude`, `codex`, `cursor`, etc.) and streams their output
- Maintains a **SQLite database** (WAL mode) of all sessions, messages, tool calls, and settings
- Watches the **filesystem** for changes via `notify` and tracks git state via `git2`
- Serves a **JSON-RPC 2.0 WebSocket server** on `ws://127.0.0.1:4300`
- Pushes **server events** to connected clients (new messages, tool calls, status changes)

### Desktop app — Flutter

The desktop app is a **thin client** — it contains UI and desktop-platform code only. All state lives in the daemon.

- Multi-pane layout: session list → chat → code editor
- Native OS menus (macOS menu bar, Windows title bar)
- Keyboard shortcuts optimized for developers
- Code editor powered by CodeMirror 6 via WebView
- Platform runners for macOS, Windows, and Linux

### Mobile app — Flutter

A companion app for monitoring and responding to sessions from a phone.

- Session list with status indicators
- Full chat view with tool-call approval
- Bottom-navigation shell optimized for touch
- Platform runners for iOS and Android

### Shared packages

| Package | Purpose |
| --- | --- |
| `clawd_proto` | Dart types mirroring the JSON-RPC protocol (`Session`, `Message`, `ToolCall`, etc.) |
| `clawd_client` | Typed WebSocket/JSON-RPC client; both apps use this to talk to the daemon |
| `clawd_core` | Shared Riverpod providers — daemon connection, session list, messages, tool calls |
| `clawd_ui` | Shared Flutter widgets — `ChatBubble`, `SessionListTile`, `ToolCallCard`, theme |

## IPC protocol

All communication between apps and daemon uses **JSON-RPC 2.0 over WebSocket**.

- **17 RPC methods** — `session.create`, `session.list`, `message.list`, `tool_call.approve`, etc.
- **7 push events** — `session.created`, `message.appended`, `tool_call.pending`, etc.
- Push events flow daemon → client as `{"jsonrpc":"2.0","method":"event.name","params":{...}}`

## Data flow (sending a message)

```
User types → MessageInput widget
  → ref.read(messageListProvider(id).notifier).send(text)
    → clawd_client.call('session.sendMessage', {...})
      → clawd daemon receives JSON-RPC request
        → spawns / resumes AI provider subprocess
          → streams output back
            → daemon pushes message.appended events
              → messageListProvider appends to state
                → ChatBubble renders new message
```
