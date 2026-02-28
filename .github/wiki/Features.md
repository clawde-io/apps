# Features

## Status key

| Symbol | Meaning |
| --- | --- |
| ✅ | Shipped — works end-to-end |
| 🟡 | Partial — code exists, needs testing or edge cases |
| 🔲 | Planned — designed, not yet built |
| 🚧 | In progress |

---

## Daemon (`clawd`)

| Feature | Status | Notes |
| --- | --- | --- |
| JSON-RPC 2.0 WebSocket server | ✅ | Port 4300 |
| Session create / list / close | ✅ | SQLite-backed |
| Message persistence | ✅ | JSONL event log + SQLite |
| Tool call lifecycle | ✅ | Pending → approve/reject → complete |
| ClaudeCodeRunner | 🟡 | Spawns `claude` subprocess; streaming not yet validated end-to-end |
| CodexRunner | 🔲 | |
| CursorRunner | 🔲 | |
| AiderRunner | 🔲 | |
| Git integration (repo status) | 🟡 | `git2` + `notify` watcher; events sent to clients |
| Drift detection | 🔲 | Validates file state against git HEAD |
| Multi-account rotation | 🔲 | Free tier: manual prompt; $9.99/yr: automatic |
| Relay client (mTLS) | 🔲 | Outbound tunnel to `api.clawde.io` |
| Auto-update | 🔲 | Checks GitHub Releases every 24h |
| Provider onboarding wizard | 🔲 | Detects installed providers, sets up accounts |
| Model Intelligence | 🟡 | Auto-selects the best model per task, tracks token usage, enforces budget caps — see [Features/Model-Intelligence](Features/Model-Intelligence) |

## Desktop app

| Feature | Status | Notes |
| --- | --- | --- |
| Flutter project scaffold | ✅ | macOS / Windows / Linux runners |
| Multi-pane layout | 🔲 | Session list + chat + editor |
| Chat view | 🔲 | Uses `clawd_ui` ChatBubble |
| Tool call approval panel | 🔲 | Uses `clawd_ui` ToolCallCard |
| CodeMirror 6 editor | 🔲 | WebView integration |
| Native macOS menus | 🔲 | |
| Keyboard shortcuts | 🔲 | |
| System tray | 🔲 | |

## Mobile app

| Feature | Status | Notes |
| --- | --- | --- |
| Flutter project scaffold | ✅ | iOS + Android (runners need `flutter create`) |
| Sessions list screen | 🟡 | UI written, needs platform runners |
| Session detail (chat) | 🟡 | UI written, needs platform runners |
| Tool call approval (sheet) | 🟡 | Modal bottom sheet |
| Settings screen | 🟡 | |

## Shared packages

| Package | Feature | Status |
| --- | --- | --- |
| `clawd_proto` | All protocol types | ✅ |
| `clawd_client` | WebSocket/JSON-RPC client | ✅ |
| `clawd_core` | Daemon connection provider | 🟡 |
| `clawd_core` | Session list + active session | 🟡 |
| `clawd_core` | Message list (family) | 🟡 |
| `clawd_core` | Tool call list (family) | 🟡 |
| `clawd_ui` | Theme + design tokens | 🟡 |
| `clawd_ui` | ChatBubble | 🟡 |
| `clawd_ui` | SessionListTile | 🟡 |
| `clawd_ui` | ToolCallCard | 🟡 |
| `clawd_ui` | MessageInput | 🟡 |
| `clawd_ui` | ConnectionStatusIndicator | 🟡 |
| `clawd_ui` | ProviderBadge | 🟡 |

> Packages are 🟡 Partial until validated with a running daemon.
