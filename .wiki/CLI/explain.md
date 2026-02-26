# clawd explain

Ask the AI to explain a file, code range, stdin input, or error message — directly from the terminal. Creates an ephemeral AI session that is not persisted after the command exits.

## Usage

```sh
clawd explain src/main.rs                   # explain the whole file
clawd explain src/main.rs --line 42         # explain around line 42 (±10 lines)
clawd explain src/main.rs --lines 40-60     # explain lines 40–60
clawd explain --stdin                       # read code from stdin
clawd explain --error "E0308 ..."           # explain a compiler/runtime error
clawd explain src/lib.rs --format json      # structured JSON output
clawd explain src/main.rs --provider codex  # use a specific AI provider
```

## Options

| Flag | Description |
| --- | --- |
| `<file>` | File to explain (positional argument) |
| `--line <n>` | Focus on a specific line (1-based); shows ±10 lines of context |
| `--lines <start-end>` | Focus on a line range, e.g. `40-60` |
| `--stdin` | Read code from stdin instead of a file |
| `--error <msg>` | Explain a compiler or runtime error message |
| `--format text` | Plain text output (default) |
| `--format json` | Structured JSON: `{explanation, suggestions: [{action, code}]}` |
| `--provider <name>` | AI provider to use (default: `claude`) |

## Examples

Explain a Rust error:

```sh
cargo build 2>&1 | clawd explain --stdin
```

Explain a specific function:

```sh
clawd explain src/session.rs --lines 120-145
```

Get structured JSON for IDE integration:

```sh
clawd explain src/lib.rs --format json | jq '.suggestions[].action'
```

## JSON output format

```json
{
  "explanation": "This function creates a new WebSocket session...",
  "suggestions": [
    {
      "action": "Add error handling for the timeout case",
      "code": "if timeout_elapsed { return Err(...) }"
    }
  ]
}
```

## Implementation notes

- Creates an ephemeral `session.create` session — not visible in session history
- Uses existing `session.send` + `session.message.delta` streaming — no new RPC methods
- Streaming output via indicatif spinner

## See also

- [clawd chat](chat.md) — interactive AI chat in the terminal
- [CLI Reference](../CLI-Reference.md) — all clawd commands
