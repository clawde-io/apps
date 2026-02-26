# Mobile Native Features

Sprint RR adds three native-grade capabilities to the ClawDE mobile app.

## Offline Mode

When the daemon is unreachable, the mobile app serves cached data from a local SQLite database.

- Sessions and message history are synced to device storage every 2 minutes while connected
- An amber banner appears when the app is offline
- Session browsing and message reading are always available offline
- Session creation and message sending are disabled offline (require live daemon connection)
- Cache is pruned automatically after 30 days

**Files:** `mobile/lib/data/offline_cache.dart` · `mobile/lib/data/offline_sync.dart` · `mobile/lib/widgets/offline_banner.dart`

## Deep Links

Open specific sessions or tasks directly from Slack messages, emails, or other apps.

### URL Scheme

```
clawde://session/{session-id}
clawde://task/{task-id}
```

### Platform Setup

| Platform | Configuration |
| --- | --- |
| iOS | `CFBundleURLTypes` in `Info.plist` — scheme: `clawde` |
| Android | `intent-filter` in `AndroidManifest.xml` — scheme: `clawde` |

### Sharing a Session Link

Tap the share icon in the session header to copy the deep link to clipboard.

**Files:** `mobile/lib/routing/deep_link_handler.dart` · `mobile/lib/features/sessions/session_share_button.dart`

## Push Notifications

Receive notifications for session events when the app is backgrounded or closed.

### Supported Events

| Event | Notification |
| --- | --- |
| `task_complete` | "Task finished in {repo}" |
| `approval_needed` | "Tool approval required in {repo}" |
| `budget_warning` | "AI budget at {pct}%" |
| `session_error` | "Session error in {repo}" |

### Platform Support

- **iOS** — APNs via Firebase Cloud Messaging bridge
- **Android** — FCM directly

### Setup

1. The app registers a device token on first launch via `push.register` RPC
2. The daemon stores the token in the `push_tokens` table
3. The ClawDE relay reads push tokens and forwards session events to FCM/APNs

**Required vault vars (for relay backend):** `APNS_KEY_ID`, `APNS_TEAM_ID`, `FIREBASE_SERVER_KEY`

**Files:** `mobile/lib/notifications/push_handler.dart` · `daemon/src/ipc/handlers/push.rs`
