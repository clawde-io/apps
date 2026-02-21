# Local Daemon (clawded)

The always-on background service that powers all ClawDE functionality. Written in Rust for performance and reliability.

## Overview

`clawded` is a long-running process that starts at login and runs in the background. It owns the filesystem, manages sessions, runs validators, and serves the API that all UIs connect to. When you close the desktop app, the daemon keeps running — your AI sessions continue in the background.

## Capabilities

| Feature | Description |
| --- | --- |
| Background service | Runs as launchd (macOS), systemd (Linux), or Windows Service |
| Auto-start | Registered with OS service manager, starts at login |
| Local API | HTTP + WebSocket server on localhost |
| Health monitoring | `/health` and `/version` endpoints |
| SQLite storage | Repos, sessions, profiles, settings |
| JSONL event logs | Per-session append-only event streams |
| Repo registry | Track registered repos with file watchers |
| Git integration | Branch and status awareness via libgit2 |
| Graceful shutdown | Clean resource release, pending write flush |
| Configuration | `config.toml` with hot-reload support |

## CLI Commands

```bash
clawde daemon start     # Start the daemon
clawde daemon stop      # Stop the daemon
clawde daemon status    # Check if daemon is running
clawde daemon install   # Register with OS service manager
clawde daemon uninstall # Remove from OS service manager
```

## How It Works

1. On first install, `clawde daemon install` registers with your OS service manager
2. The daemon starts automatically at login
3. It opens a local HTTP + WebSocket server (default: `localhost:31415`)
4. Desktop, web, and mobile apps connect to this API
5. All state is stored locally in `~/.clawde/` (SQLite database + JSONL logs)

## Configuration

The daemon reads `~/.clawde/config.toml`:

```toml
[daemon]
port = 31415
log_level = "info"

[storage]
db_path = "~/.clawde/clawde.db"
log_path = "~/.clawde/logs/"
```
