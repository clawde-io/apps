# AI Memory

AI Memory gives ClawDE a persistent, user-controlled knowledge store that survives across sessions. The daemon injects relevant memory entries into every AI system prompt, so your preferences, project conventions, and recurring context are always present — without you having to repeat yourself.

## How It Works

1. The daemon maintains a `memory_entries` SQLite table with global and project-scoped entries
2. Before sending a message to the AI, the daemon prepends a `<clawd_memory>` block with the most relevant entries (weight-ordered, token-budgeted)
3. After a session ends, the extractor scans the conversation for new learnable facts and suggests them as memory entries
4. Users can manage entries directly in the Memory page or via the CLI

## Scopes

| Scope | Coverage | Example |
| --- | --- | --- |
| `global` | All sessions, all projects | `preferences.language = "Rust"` |
| `proj:{sha256}` | One project only | `project.style = "4-space tabs"` |

Project scope is derived from the SHA-256 hash of the repo path (first 16 chars).

## Entry Fields

| Field | Type | Description |
| --- | --- | --- |
| `key` | string | Dot-notation path — e.g. `preferences.verbosity` |
| `value` | string | The remembered value |
| `weight` | 1–10 | Priority for token-budget ordering. 10 = always included |
| `scope` | string | `global` or `proj:{sha256}` |
| `source` | string | `user` (manually added) or `ai` (extracted) |

## IPC Methods

| Method | Description |
| --- | --- |
| `memory.list` | List entries by scope or repo path |
| `memory.add` | Add or update an entry (upsert by scope+key) |
| `memory.remove` | Remove an entry by ID |
| `memory.update` | Alias for `memory.add` with upsert semantics |

## CLI

```bash
# List all global entries
clawd memory list

# List entries for a project
clawd memory list --repo-path /path/to/project

# Add a global preference
clawd memory add preferences.language Rust --weight 8

# Show a specific entry
clawd memory show preferences.language

# Remove an entry
clawd memory remove <id>
```

## Desktop App

Go to **Memory** in the sidebar to:
- Search all entries
- Add entries with key, value, and weight slider
- Delete entries with confirmation
- See the weight badge (green = high priority, amber = medium, grey = low)

The **session header** shows a memory count badge (purple `M` chip) when entries are active for the current session's repo.

## Personalization Templates

Five built-in starter templates are available at `apps/daemon/src/memory/templates/`:

| Template | Focus |
| --- | --- |
| `rust.toml` | Rust idioms, error handling, async patterns |
| `typescript.toml` | TypeScript strict mode, ESM, React patterns |
| `flutter.toml` | Flutter/Dart conventions, Riverpod, null safety |
| `fullstack.toml` | Full-stack web development preferences |
| `ml.toml` | Machine learning, Python, Jupyter conventions |

Load a template with:
```bash
clawd memory load-template rust
```

## Token Budget

The daemon limits memory injection to avoid consuming too much of the AI's context window. The default budget is 512 tokens. Entries are sorted by weight descending — higher weight entries are always included first.

## Privacy

All memory data is stored locally in the daemon's SQLite database (`~/.claw/clawd.db`). Nothing is sent to ClawDE servers. In air-gap mode, memory works identically — it never leaves your machine.
