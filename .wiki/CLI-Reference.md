# CLI Reference

Complete reference for all `clawd` command-line commands.

`clawd` is the ClawDE daemon binary. It manages AI sessions, git repo state, tasks, drift detection, and connectivity. On macOS and Linux it runs as a background process; the Flutter desktop app starts it automatically.

## Global flags

| Flag | Default | Description |
| --- | --- | --- |
| `--config <path>` | `~/.clawd/config.toml` | Path to the config file |
| `--data-dir <path>` | `~/.clawd/data/` | Directory for SQLite DB and auth token |
| `--port <n>` | `4300` | WebSocket / HTTP port |
| `--bind <addr>` | `127.0.0.1` | Bind address (`0.0.0.0` for LAN access) |
| `--log-level <level>` | `info` | `trace` / `debug` / `info` / `warn` / `error` |
| `--log-format <fmt>` | `text` | `text` / `json` |
| `--version` | — | Print the daemon version and exit |
| `--help` | — | Print help |

---

## Commands

### clawd start

Start the daemon in the foreground.

```sh
clawd start
clawd start --port 4300 --bind 127.0.0.1
clawd start --log-level debug
```

The daemon starts, prints its version, binds the WebSocket server, and writes the auth token to `{data-dir}/auth_token`.

To run in the background as a service, see [Getting Started](Getting-Started.md).

---

### clawd stop

Send SIGTERM to a running daemon and wait for it to shut down cleanly.

```sh
clawd stop
```

The daemon drains active sessions, checkpoints the WAL journal, and exits.

---

### clawd status

Print the status of the running daemon.

```sh
clawd status
```

Example output:

```
clawd v0.2.0 — running (PID 12345)
Port:           4300
Sessions:       3 active / 0 paused
Data dir:       ~/.clawd/data/
Auth:           enabled
Relay:          connected (api.clawde.io)
License:        Personal Remote ($9.99/yr) — valid
Last updated:   2026-02-25 (up to date)
```

---

### clawd doctor

Run the 8-point health diagnostics and print results.

```sh
clawd doctor
clawd doctor --fix        # attempt to auto-fix warnings
clawd doctor --json       # output as JSON
```

Checks run:

1. Daemon process (is clawd running and responsive)
2. Port availability (is port 4300 reachable)
3. SQLite integrity (`PRAGMA integrity_check`)
4. Auth token (exists, correct permissions)
5. Providers (at least one AI CLI installed and authenticated)
6. Disk space (data dir has > 100 MB free)
7. File permissions (data dir is readable/writable)
8. Keychain (auth token accessible from keychain if used)

---

### clawd session

Manage sessions from the command line.

```sh
# List all sessions
clawd session list

# Create a new session
clawd session create --provider claude --repo ~/Sites/myproject

# Show session details
clawd session get <session-id>

# Delete a session
clawd session delete <session-id>

# Send a message
clawd session send <session-id> "Explain the architecture of this codebase"

# Show recent messages
clawd session messages <session-id> --limit 20
```

---

### clawd task

Manage tasks in the task queue.

```sh
# List all tasks
clawd task list

# List tasks by status
clawd task list --status pending
clawd task list --status in_progress

# Show a task
clawd task get <task-id>

# Add a task
clawd task add --title "Fix login bug" --repo ~/Sites/myapp

# Claim the next available task (for an agent)
clawd task claim --agent <agent-id>

# Mark a task done
clawd task done <task-id> --notes "Fixed the null pointer in auth.rs"

# Import tasks from a planning markdown file
clawd task import --file .claude/tasks/active.md

# Export tasks to JSON
clawd task export --format json > tasks.json
```

---

### clawd repo

Manage git repositories.

```sh
# List open repos
clawd repo list

# Open a repo
clawd repo open ~/Sites/myproject

# Show repo status
clawd repo status <repo-id>

# Run drift scanner
clawd repo drift ~/Sites/myproject

# Show drift report
clawd repo drift-report ~/Sites/myproject
```

---

### clawd provider

Manage and test AI providers.

```sh
# List detected providers
clawd provider list

# Check if a provider is available
clawd provider check claude

# Add an API key
clawd provider add-key --provider codex --key sk-...

# Test a provider (sends a minimal ping message)
clawd provider test --provider claude
```

---

### clawd account

Manage multi-account switching.

```sh
# List accounts
clawd account list

# Add an account
clawd account add --provider claude --key sk-ant-...

# Set account priority
clawd account priority <account-id> --priority 1

# View account usage history
clawd account history <account-id>

# Delete an account
clawd account delete <account-id>
```

---

### clawd update

Manage daemon updates.

```sh
# Check for updates
clawd update check

# Apply the latest update
clawd update apply

# Show current update policy
clawd update policy

# Set update policy
clawd update policy --set auto    # auto-apply on idle
clawd update policy --set notify  # notify only
clawd update policy --set off     # disable checks
```

---

### clawd relay

Manage the remote relay connection (Personal Remote and Cloud tiers).

```sh
# Show relay status
clawd relay status

# Force reconnect
clawd relay reconnect

# Show relay connection info
clawd relay info
```

Relay requires a valid Personal Remote or Cloud license. On Free tier, remote access is not available.

---

### clawd pair

Pair a mobile or secondary device.

```sh
# Generate a pairing PIN
clawd pair generate

# List paired devices
clawd pair list

# Revoke a device
clawd pair revoke <device-id>
```

---

### clawd license

Manage the license.

```sh
# Show current license
clawd license show

# Activate a license key
clawd license activate <key>

# Refresh the license from the server
clawd license refresh
```

---

### clawd worktree

Manage per-task git worktrees.

```sh
# List all worktrees
clawd worktree list

# Create a worktree for a task
clawd worktree create <task-id>

# Show worktree diff
clawd worktree diff <task-id>

# Accept changes (merge to base branch)
clawd worktree accept <task-id>

# Reject changes (discard)
clawd worktree reject <task-id>

# Clean up orphaned worktrees
clawd worktree cleanup
```

---

### clawd pack

Manage packs from the marketplace.

```sh
# Search packs
clawd pack search "rust formatter"

# Install a pack
clawd pack install rust-fmt

# Update all packs
clawd pack update

# Remove a pack
clawd pack remove rust-fmt

# List installed packs
clawd pack list
```

---

### clawd afs

Manage AFS (AI Filesystem) for a workspace.

```sh
# Initialise AFS in a project
clawd afs init ~/Sites/myproject

# Show AFS status
clawd afs status ~/Sites/myproject

# Sync instruction files
clawd afs sync ~/Sites/myproject
```

---

### clawd lsp

Manage Language Server Protocol integrations.

```sh
# Start an LSP server for a language
clawd lsp start --language rust --root ~/Sites/myproject

# List running LSP servers
clawd lsp list

# Stop an LSP server
clawd lsp stop <server-id>
```

---

## Environment variables

| Variable | Description |
| --- | --- |
| `CLAWD_DATA_DIR` | Override the data directory (same as `--data-dir`) |
| `CLAWD_PORT` | Override the port (same as `--port`) |
| `CLAWD_LOG_LEVEL` | Override the log level |
| `CLAWD_AUTH_TOKEN` | Override the auth token (set at startup; reading from file is preferred) |
| `CLAWD_RELAY_URL` | Override the relay URL (default: `wss://api.clawde.io/relay`) |
| `CLAWD_NO_COLOR` | Disable colour in terminal output |

---

## Config file format

`~/.clawd/config.toml`:

```toml
# clawd configuration file

[daemon]
port = 4300
bind_address = "127.0.0.1"
log_level = "info"

[security]
# Maximum connections per IP per minute (loopback exempt)
max_connections_per_minute_per_ip = 30
# Maximum RPC calls per connection per minute (loopback exempt)
max_rpc_calls_per_minute = 300

[relay]
enabled = true
url = "wss://api.clawde.io/relay"
# Reconnect backoff (seconds)
initial_backoff = 2
max_backoff = 60

[update]
# "auto" | "notify" | "off"
policy = "auto"
# Check interval in seconds (default: 86400 = 24h)
check_interval = 86400

[storage]
# WAL autocheckpoint threshold (pages)
wal_autocheckpoint = 4096
# Cache size in KiB (negative = kibibytes)
cache_size_kb = 32768
```

---

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Port already in use |
| 4 | Database error (integrity check failed) |
| 5 | Auth token error |

---

## See also

- [Getting Started](Getting-Started.md) — installation and first session
- [RPC Reference](RPC-Reference.md) — full JSON-RPC 2.0 API
- [Architecture](Architecture.md) — how the daemon, relay, and clients fit together
- [Configuration](Configuration.md) — detailed config file reference
