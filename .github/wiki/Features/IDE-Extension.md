# IDE Extension

The ClawDE IDE Extension connects your editor directly to the local daemon, giving you inline AI assistance without leaving your coding environment.

## Supported editors

| Editor | Extension | Status |
| --- | --- | --- |
| VS Code | `clawde.clawde-vscode` | Planned Sprint LL |
| JetBrains (all IDEs) | `io.clawde.plugin` | Planned Sprint MM |
| Neovim | `clawde.nvim` | Community — post v0.3.0 |
| Zed | Native extension | Planned post v0.3.0 |

## What the extension does

- **Inline chat**: Open a chat panel scoped to the current file or selection
- **Diff preview**: Proposed changes appear as inline diff before applying
- **Task status**: See active session status in the status bar
- **Error fix**: Right-click a diagnostic → "Fix with ClawDE"
- **Context picker**: Choose which open files go into the prompt

## How it connects

The extension talks to the daemon at `ws://127.0.0.1:4300` using the same JSON-RPC 2.0 protocol as the desktop app. No separate process needed — the daemon must be running.

## Authentication

The extension uses the same device token as the desktop app. If the desktop app is installed, the extension reuses its token automatically.

## Status

VS Code extension is Sprint LL. Requires stable RPC protocol (locked in v0.2.0) and VS Code Webview for the chat panel.

## Related

- [Daemon Reference](../Daemon-Reference.md)
- [RPC Reference](../RPC-Reference.md)
- [Getting Started](../Getting-Started.md)
