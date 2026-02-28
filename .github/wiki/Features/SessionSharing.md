# Session Sharing

> Sprint EE CS — Real-time collaborative session sharing.

## Overview

Session sharing lets you invite other devices or users to view a live clawd session. Shares are token-based, expire automatically, and can be revoked at any time.

## How It Works

1. Create a share token for a session (`session.share`)
2. Send the token to the viewer (out of band — clipboard, message, etc.)
3. Viewer opens the session using the token — read-only access
4. Revoke the token at any time (`session.revokeShare`)

## RPC Methods

| Method | Description |
| --- | --- |
| `session.share` | Create a share token for a session |
| `session.revokeShare` | Revoke an active share token |
| `session.shareList` | List active shares for a session |

### session.share

```json
{ "session_id": "abc123", "expires_in": 3600 }
```

Returns:
```json
{
  "shareToken": "clw_share_...",
  "expiresAt": "2026-02-26T18:00:00Z"
}
```

### session.revokeShare

```json
{ "share_token": "clw_share_..." }
```

Returns: `{ "ok": true }`

### session.shareList

```json
{ "session_id": "abc123" }
```

Returns:
```json
{
  "shares": [
    {
      "shareToken": "clw_share_...",
      "sessionId": "abc123",
      "expiresAt": "2026-02-26T18:00:00Z",
      "createdAt": "2026-02-26T14:00:00Z"
    }
  ]
}
```

## Database Schema

Shares are stored in the `session_shares` table:

```sql
CREATE TABLE session_shares (
    id          TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL,
    share_token TEXT NOT NULL UNIQUE,
    expires_at  TEXT,
    revoked_at  TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

A share is **active** when `revoked_at IS NULL AND (expires_at IS NULL OR expires_at > datetime('now'))`.

## Mobile UI

Open any session → tap the share button → create a token → send it to a viewer.

The **Shared Session** screen lists all active tokens with time-to-expiry and a revoke button.

## Security Notes

- Tokens use `clw_share_` prefix for easy identification
- Viewers have read-only access — they cannot send messages or modify the session
- Always set a reasonable expiry (`expires_in`) — avoid indefinite shares
- Revoke tokens immediately when collaboration ends
