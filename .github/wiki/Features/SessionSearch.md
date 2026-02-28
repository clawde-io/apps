# Session Search

ClawDE indexes all session messages in a SQLite FTS5 virtual table, enabling fast full-text search across your entire conversation history.

## Keyboard Shortcut

**Cmd+K** (macOS) / **Ctrl+K** (Windows/Linux) — opens the global search overlay from anywhere in the desktop app.

## Search UI

The search bar opens as a modal dialog and searches as you type (200 ms debounce). Results are ranked by BM25 relevance and show:

- The message role icon (user / assistant)
- A text snippet with the matching phrase
- The date and time of the message
- Arrow keys to navigate, Enter to open, Escape to close

Clicking a result navigates to that session and scrolls to the relevant message.

## Filters

Advanced filters can be passed programmatically via the RPC:

| Filter | Description |
| --- | --- |
| `sessionId` | Restrict to a specific session |
| `dateFrom` | Messages created at or after this ISO-8601 timestamp |
| `dateTo` | Messages created before or at this ISO-8601 timestamp |
| `role` | `"user"` or `"assistant"` |

## BM25 Ranking

Results are ordered by BM25 rank (lower rank = more relevant). The FTS5 tokenizer uses Porter stemming (`porter unicode61`), so searching "run" also matches "running", "runs", etc.

## RPC Reference

### `session.search`

Request:

```json
{
  "query": "code completion",
  "limit": 20,
  "filterBy": {
    "sessionId": "optional-session-uuid",
    "dateFrom": "2026-01-01T00:00:00Z",
    "dateTo": "2026-12-31T23:59:59Z",
    "role": "user"
  }
}
```

Response:

```json
{
  "results": [
    {
      "sessionId": "abc123",
      "messageId": "msg456",
      "snippet": "…the <b>code completion</b> engine…",
      "role": "assistant",
      "createdAt": "2026-03-01T14:30:00Z",
      "rank": -5.2
    }
  ],
  "totalHits": 1
}
```

Snippet highlights use `<b>…</b>` HTML tags. Strip them in plain-text contexts.

## Implementation

- SQLite FTS5 virtual table `session_fts` (migration 028)
- Auto-populated by an `AFTER INSERT` trigger on the `messages` table
- Backfilled at migration time for existing messages
- `DELETE` trigger keeps the index clean when messages are removed
