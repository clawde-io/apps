# Project Pulse

Project Pulse gives you a 7-day view of what changed semantically in your codebase — features added, bugs fixed, code refactored, tests written.

## Overview

Every time you commit through a ClawDE session, the daemon classifies the change and records it as a semantic event. Project Pulse aggregates those events into a velocity dashboard.

This is different from `git log`. Git tells you _what_ changed. Pulse tells you _why kind of work_ you've been doing.

## Velocity categories

| Category | What counts |
| -------- | ----------- |
| Features | Commits mentioning "add", "implement", "create", "feature" |
| Bug Fixes | Commits mentioning "fix", "bug", "issue", "resolve", "patch" |
| Refactors | Commits mentioning "refactor", "clean", "reorganize", "rename" |
| Tests | Commits mentioning "test", or changes to `test_*`, `spec_*` files |
| Config | Changes to `*.toml`, `*.yaml`, `*.json`, `.env` files |
| Dependencies | Changes to `Cargo.lock`, `pubspec.lock`, `package-lock.json` |

When a commit matches multiple categories, the highest-priority one wins (dependencies > config > tests > bug fixes > refactors > features).

## Viewing the pulse

From the desktop app: open **Project Pulse** from the left nav.

The screen shows:
- Four velocity cards (Features / Bug Fixes / Refactors / Tests) with counts for the last 7 days
- A scrollable feed of recent semantic events with timestamps and affected files

## RPC reference

```
project.pulse { days: 7 }
```

Response:

```json
{
  "period": "7d",
  "velocity": {
    "features": 4,
    "bugs": 2,
    "refactors": 1,
    "tests": 3,
    "configs": 0,
    "dependencies": 1
  },
  "events": [
    {
      "id": "...",
      "eventType": "feature_added",
      "summaryText": "Add workflow recipe engine",
      "affectedFiles": ["src/workflows/engine.rs"],
      "createdAt": "2026-02-26T12:00:00Z"
    }
  ]
}
```

## Dart types

`ProjectPulse`, `ProjectVelocity`, and `SemanticEvent` are available in `clawd_proto`.

The `pulse7dProvider` Riverpod provider in `clawd_core` fetches the 7-day pulse automatically.
