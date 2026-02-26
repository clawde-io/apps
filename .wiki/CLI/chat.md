# clawd chat

Interactive AI chat directly in your terminal. Connects to the running daemon via JSON-RPC WebSocket and streams AI responses in real time.

## Usage

```sh
clawd chat                              # new interactive session
clawd chat --resume <session-id>        # resume an existing session
clawd chat --session-list               # pick from recent sessions
clawd chat --non-interactive "prompt"   # single-shot query, print response, exit
clawd chat --provider codex             # use a specific AI provider
```

## Interactive mode

The full-screen terminal UI shows:

- **Header** — session ID and current status (streaming indicator)
- **Message history** — scrollable list of user and assistant messages
- **Input bar** — type your message and press Enter to send
- **Tool calls** — collapsed by default; press `t` to expand the last tool call

### Keyboard shortcuts

| Key | Action |
| --- | --- |
| `Enter` | Send message |
| `Ctrl+C` | Pause session and exit |
| `t` | Toggle last tool call expansion |
| Type `exit` or `quit` + Enter | End session and exit |

## Non-interactive mode

Send a single prompt and print the response — useful for scripting:

```sh
clawd chat --non-interactive "Explain async/await in Rust"
clawd chat --non-interactive "$(cat error.txt)"
```

Exit code 0 on success, non-zero on error.

## Resume a session

```sh
# Resume by ID
clawd chat --resume sess-abc123

# Pick from recent sessions interactively
clawd chat --session-list
```

## Implementation notes

- Uses existing `session.create` / `session.send` RPC methods — no new RPC methods added
- Streaming via `session.message.delta` push events
- `session.message.complete` signals the end of a response
- Terminal UI: ratatui + crossterm
- Spinner: indicatif

## See also

- [clawd explain](explain.md) — explain a file or error in the terminal
- [RPC Reference](../RPC-Reference.md) — session.* methods
- [CLI Reference](../CLI-Reference.md) — all clawd commands
