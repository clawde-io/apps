# Integrations

ClawDE connects to the tools and editors you already use. All integrations talk to the local `clawd` daemon over the standard JSON-RPC 2.0 WebSocket API.

## Available Integrations

| Integration | Type | Status |
| --- | --- | --- |
| [GitHub App](GitHub.md) | CI + Code Review | Available |
| [JetBrains Plugin](JetBrains.md) | IDE (IntelliJ, WebStorm, PyCharm…) | Available |
| [Neovim Plugin](Neovim.md) | Editor | Available |
| VS Code Extension | Editor | Planned (Sprint PP) |
| Slack App | Notifications | Planned (Sprint TT) |

## How Integrations Work

Every integration is a thin client that:

1. Connects to `clawd` at `ws://localhost:4300` (local) or via the relay (remote)
2. Authenticates with an auth token from `~/.claw/auth.token`
3. Uses the standard [JSON-RPC 2.0 API](../API/Overview.md) — same methods as the Flutter app

No integration has special privileges. They all use the same public daemon API.

## Building Your Own

The daemon API is fully documented. Any language that supports WebSocket can build a ClawDE integration.

- [API Overview](../API/Overview.md)
- [Auth](../API/Auth.md)
- [Session Methods](../API/Sessions.md)

The `clawd_client` Dart package and the `@clawde/sdk` TypeScript package are reference implementations you can use as a guide.
