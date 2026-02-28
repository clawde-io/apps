# Mobile Daily Digest

> Sprint EE DD — Daily session summary delivered to your mobile device.

## Overview

The Daily Digest gives you a one-screen summary of the day's AI sessions: tasks completed, sessions run, files touched, and an expandable view of each session.

## Screen Layout

```
┌─────────────────────────────────┐
│  Daily Digest          🔄       │
├─────────────────────────────────┤
│  ┌───────────────────────────┐  │
│  │ Today                     │  │
│  │ [4 Sessions] [12 Done] [2 In Progress] │
│  │ main.dart, auth.rs, ...   │  │
│  └───────────────────────────┘  │
│                                 │
│  Sessions                       │
│  ┌───────────────────────────┐  │
│  │ 💬 Auth refactor  ›       │  │
│  │ claude · 34 messages · 5 tasks │
│  └───────────────────────────┘  │
│  ┌───────────────────────────┐  │
│  │ 💬 Fix build errors  ↓   │  │
│  │ codex · 12 messages · 3 tasks │
│  │  Files changed:            │  │
│  │  Cargo.toml               │  │
│  │  src/lib.rs               │  │
│  └───────────────────────────┘  │
└─────────────────────────────────┘
```

## RPC

The digest screen calls `digest.today` with no parameters:

```json
{}
```

Returns:
```json
{
  "date": "2026-02-26",
  "metrics": {
    "sessionsRun": 4,
    "tasksCompleted": 12,
    "tasksInProgress": 2,
    "topFiles": ["main.dart", "auth.rs", "schema.sql"],
    "evalAvg": 0.0,
    "velocity": {}
  },
  "sessions": [
    {
      "sessionId": "abc123",
      "sessionTitle": "Auth refactor",
      "provider": "claude",
      "messagesCount": 34,
      "tasksCompleted": 5,
      "filesChanged": [],
      "startedAt": "2026-02-26T09:00:00Z",
      "endedAt": "2026-02-26T10:30:00Z"
    }
  ]
}
```

## Push Notifications

When the daemon runs the 6pm digest job, it fires a push notification to registered mobile devices:

> **Daily Digest** — 4 sessions · 12 tasks done · 3 files touched

Tap the notification to open the Digest screen directly.

## Navigation

The Digest screen is accessible from:
- The mobile home tab bar (digest icon)
- Tapping a daily digest push notification

## Refresh

Pull to refresh or tap the refresh button in the app bar to re-fetch the current day's digest.
