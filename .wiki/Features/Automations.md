# Automations

ClawDE automations are trigger-action rules that fire automatically when events happen in the daemon. Use them to run tests after sessions, extract TODOs as tasks, or notify you when a long session completes.

## Trigger types

| Trigger | When it fires |
| --- | --- |
| `session_complete` | An AI session ends (completed or errored) |
| `task_done` | A task's status changes to "done" |
| `file_saved` | A file is written by a tool call |
| `cron` | A cron expression fires |

## Action types

| Action | What it does |
| --- | --- |
| `run_tests` | Runs a shell command (e.g. `cargo test`) and pushes results as `automation.testResults` |
| `send_notification` | Pushes `automation.notification` to all connected clients |
| `create_task` | Creates a new task in the task store |
| `run_script` | Runs an arbitrary shell script |

## Config — `.claw/config.toml`

```toml
[[automations]]
name        = "run-tests-after-session"
description = "Run tests when a session completes"
trigger     = "session_complete"
enabled     = true
action      = "run_tests"

[automations.action_config]
command = "cargo test --quiet"
```

Multiple automations can share the same trigger type. They all fire independently.

## Built-in automations

Three automations are built-in and always registered. They can be disabled in the UI or config but not deleted.

| Name | Trigger | Default |
| --- | --- | --- |
| `run-tests-on-complete` | `session_complete` | Disabled |
| `todo-extractor` | `session_complete` | Enabled |
| `long-session-notifier` | `session_complete` + `session_duration_secs>300` | Enabled |

## Condition expressions

Simple conditions filter when an automation fires:

| Expression | Meaning |
| --- | --- |
| `session_duration_secs>300` | Only fires when session ran >5 minutes |
| `file_ext=.rs` | Only fires for Rust file changes |

Leave `condition` empty (or omit it) to fire on every matching trigger.

## RPC methods

| Method | Description |
| --- | --- |
| `automation.list` | List all automations with status |
| `automation.trigger` | Manually trigger an automation (useful for testing) |
| `automation.disable` | Enable or disable an automation by name |

## Push events

- `automation.testResults` — fired after `run_tests` action completes
- `automation.notification` — fired by `send_notification` action

## Example: TODO extractor

When a session completes, the `todo-extractor` automation scans the session's AI output for lines containing `TODO:` and creates a task for each one. This turns in-conversation TODOs into tracked tasks automatically.
