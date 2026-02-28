# Background Mode

ClawDE runs as a background daemon (`clawd`) that keeps AI sessions alive even when the desktop app window is closed. The system tray icon lets you access it from anywhere.

## How it works

When you close the ClawDE window, the daemon keeps running. Your active sessions stay paused and resume instantly when you reopen the app. The tray icon shows the current daemon state at a glance.

| Icon state | Meaning |
| --- | --- |
| ● Running | Daemon running, no relay clients connected |
| ● Connected | At least one remote device is connected via relay |
| ⚠ Error | Daemon failed to start or encountered an unrecoverable error |

## System tray menu

Right-click (or left-click on macOS) to open the tray menu:

- **● Running / Connected / Error** — current status (read-only)
- **Sessions list** — up to 5 recent sessions with status icons (▶ running, ⏸ paused)
- **+ New Session** — open the app and start a new session
- **Show ClawDE** — bring the window to focus
- **Quit ClawDE** — shut down the daemon and exit

## Settings

Open **Settings > General** to configure background mode:

| Setting | Default | Description |
| --- | --- | --- |
| Keep running when window is closed | On | Daemon runs in background; tray icon stays visible |
| Start at login | Off | Installs a platform service so the daemon starts automatically |

## Start at login

When enabled, ClawDE installs a platform service so the daemon starts automatically at login/boot:

| Platform | Service location |
| --- | --- |
| macOS | `~/Library/LaunchAgents/com.clawde.clawd.plist` |
| Linux | `~/.config/systemd/user/clawd.service` |
| Windows | Windows Service via NSSM (requires admin for initial install) |

You can also manage this from the CLI:

```sh
clawd service install    # enable start-at-login
clawd service uninstall  # disable start-at-login
clawd service status     # check if installed
```

## Notifications

When a session completes a task, needs approval, or hits a budget warning, ClawDE surfaces a notification via the tray. Clicking the notification:

1. Brings the ClawDE window to focus
2. Navigates to the relevant session

## Reconnection

After hiding the window and reopening the app, ClawDE reconnects to the running daemon in under 500 ms. Sessions appear exactly as you left them — no reload, no lost context.
