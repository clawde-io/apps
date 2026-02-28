# Session Replay

Session Replay lets you export any ClawDE session to a portable file, share it with teammates, and replay it at any speed in the desktop app.

## What it does

A session export captures the full conversation — messages, tool calls, and code changes — into a single compressed file. Anyone with a ClawDE desktop app can import that file and watch the session play back.

Useful for:
- Reviewing how a complex feature was built
- Onboarding new teammates to a codebase
- Debugging a session that went wrong
- Sharing AI-assisted work across teams

## Exporting a session

From the desktop app: open a session and tap the export button (download icon) in the toolbar. The file is saved to your Downloads folder as `clawd-session-<id>.clawd`.

From the CLI:

```sh
clawd session export <session-id>
clawd session export <session-id> --out my-session.clawd
```

## Importing a session

From the CLI:

```sh
clawd session import my-session.clawd
```

This creates a _replay session_ — a read-only copy of the original session. Open the desktop app to watch it replay.

## Replay controls

The Session Replay screen in the desktop app shows:
- Play / Pause button
- Speed selector: 0.5x, 1x, 2x, 5x
- Progress indicator (current message / total messages)
- Re-export button to save the bundle to a new file

## File format

Session bundles are gzip-compressed JSON, base64-encoded. The `.clawd` extension is used by convention. The file is human-readable after decompressing:

```json
{
  "version": 1,
  "originalSessionId": "...",
  "exportedAt": "2026-02-26T12:00:00Z",
  "session": { ... },
  "messages": [ ... ]
}
```

## RPC reference

| Method | Description |
| ------ | ----------- |
| `session.export` | Export a session to a bundle |
| `session.import` | Import a bundle and create a replay session |
| `session.replay` | Start replaying a session at a given speed |

## CLI reference

```
clawd session export <session-id> [--out <file>]
clawd session import <file>
```

## Dart types

`SessionBundle`, `ImportResult`, and `ReplaySession` are available in `clawd_proto`.
