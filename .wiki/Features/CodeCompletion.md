# Code Completion

ClawDE provides inline AI-powered code completion using a fill-in-middle (FIM) approach. The completion engine integrates directly with the daemon and surfaces suggestions as ghost text in the editor.

## How It Works

When the cursor is idle for 150 ms (debounce), the editor sends the text before and after the cursor (prefix + suffix) to the `completion.complete` RPC. The daemon:

1. Checks the LRU cache (256 entries, key = SHA-256 of last-512 bytes of prefix + first-128 bytes of suffix).
2. On miss, extracts repo context (import statements + nearest function signature, capped at 512 tokens).
3. Builds the FIM prompt and forwards to the configured provider.
4. Caches the result and returns the insertion text.

## FIM Prompt Format

```
Complete the missing code. Language: Rust.
Return ONLY the inserted text — no markdown fences, no explanation.
<|fim_prefix|>{prefix}<|fim_suffix|>{suffix}<|fim_middle|>
```

The provider fills in the `<|fim_middle|>` token.

## Ghost Text UX

- The first line of the suggestion appears in grey italic text below the cursor.
- **Tab** — accept the full suggestion and insert it at the cursor.
- **Escape** — dismiss the suggestion without inserting.
- A spinner in the top-right corner appears while a request is in flight.

## Configuration

`[completion]` section in `~/.claw/config.toml`:

```toml
[completion]
enabled     = true
debounce_ms = 150
max_tokens  = 64
provider    = "codex-spark"   # or "claude-haiku"
```

| Field | Default | Description |
| --- | --- | --- |
| `enabled` | `true` | Enable/disable completions globally |
| `debounce_ms` | `150` | Delay before sending request (milliseconds) |
| `max_tokens` | `64` | Max tokens to generate per completion |
| `provider` | `"codex-spark"` | Provider for completions; `"claude-haiku"` also supported |

## RPC Reference

### `completion.complete`

Request:

```json
{
  "filePath": "src/main.rs",
  "prefix": "fn add(a: i32, b: i32) -> i32 {\n    ",
  "suffix": "\n}",
  "cursorLine": 1,
  "cursorCol": 4,
  "fileContent": "full file text (optional, for context injection)",
  "sessionId": "session-uuid"
}
```

Response:

```json
{
  "insertions": [
    { "text": "a + b", "startLine": 1, "endLine": 1, "confidence": 0.9 }
  ],
  "source": "provider"
}
```

`source` is `"cache"` when the result was served from the LRU cache.

## Cache Metrics

Hit rate and entry count are exposed via `analytics.budget` as `completion_cache_hit_rate` and `completion_cache_entries`.
