# Configuration

ClawDE reads configuration from `clawd.toml`. The file location varies by platform:

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/clawd/clawd.toml` |
| Linux | `~/.local/share/clawd/clawd.toml` or `$XDG_DATA_HOME/clawd/clawd.toml` |
| Windows | `%APPDATA%\clawd\clawd.toml` |

If the file doesn't exist, ClawDE uses defaults. All settings are optional.

## All configuration keys

### General

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `log_level` | string | `"info"` | Log verbosity: `trace`, `debug`, `info`, `warn`, `error` |
| `max_sessions` | integer | `0` | Max concurrent sessions. 0 = unlimited |
| `port` | integer | `4300` | WebSocket IPC server port |

### Resources

Controls how the daemon manages memory across multiple sessions.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `resources.max_memory_percent` | integer | `70` | Max % of total system RAM for daemon + sessions (10-95) |
| `resources.max_concurrent_active` | integer | `0` | Max active CLI subprocesses. 0 = auto-calculate from RAM |
| `resources.idle_to_warm_secs` | integer | `120` | Seconds before idle session is frozen (SIGSTOP) |
| `resources.warm_to_cold_secs` | integer | `300` | Seconds before frozen session is killed and saved to disk |
| `resources.process_pool_size` | integer | `1` | Pre-warmed CLI workers for fast cold resume |
| `resources.emergency_memory_percent` | integer | `90` | RAM % that triggers aggressive eviction |
| `resources.poll_interval_secs` | integer | `5` | How often the resource governor checks system memory |

### Provider settings

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `provider.claude.timeout_secs` | integer | `300` | Session timeout for Claude provider |
| `provider.codex.timeout_secs` | integer | `300` | Session timeout for Codex provider |

## Environment variables

Every config key can be overridden with an environment variable using `CLAWD_` prefix:

| Variable | Config key | Example |
|----------|-----------|---------|
| `CLAWD_PORT` | `port` | `CLAWD_PORT=4301` |
| `CLAWD_LOG` | `log_level` | `CLAWD_LOG=debug` |
| `CLAWD_DATA_DIR` | Data directory | `CLAWD_DATA_DIR=/custom/path` |
| `CLAWD_MAX_SESSIONS` | `max_sessions` | `CLAWD_MAX_SESSIONS=5` |

## Example configuration

```toml
# Tuned for a 16 GB machine with moderate AI usage
[resources]
max_memory_percent = 65
idle_to_warm_secs = 60
warm_to_cold_secs = 180
process_pool_size = 1

[provider.claude]
timeout_secs = 600   # 10-minute timeout for long tasks
```

## Hot reload

ClawDE watches `clawd.toml` for changes and applies non-critical settings (like `log_level`) without a daemon restart. Session limits and port changes require a restart.
