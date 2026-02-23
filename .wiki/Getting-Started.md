# Getting Started

Get from download to your first AI session in under 5 minutes.

## Prerequisites

- macOS 13+, Windows 10+, or Ubuntu 22.04+
- [Claude Code](https://claude.ai/code) installed and authenticated (for Claude sessions)

## Step 1: Download ClawDE

Download the latest release from [GitHub Releases](https://github.com/clawde-io/apps/releases).

- **macOS:** `.dmg` → drag to Applications
- **Linux:** `.tar.gz` → extract, run `./clawd service install`
- **Windows:** `.msi` → run installer

**macOS note:** Right-click the app → Open the first time (Gatekeeper prompt). After that, it opens normally.

## Step 2: First Launch

The app checks that your system is ready:

1. **Daemon starts automatically** — `clawd` runs in the background on port 4300
2. **Provider check** — the app detects Claude Code. If not found, follow the install guide shown
3. **Open a project** — select a folder containing your code (must be a git repo)
4. **Start your first session** — type a prompt and press Enter

If anything fails, run `clawd doctor` in your terminal to diagnose.

## Step 3: Connect Remote Devices (Optional)

To use ClawDE from your phone or another computer:

1. Open Settings → Remote Access → Add Device on the host machine
2. Scan the QR code or enter the 6-digit PIN on your other device
3. For access from anywhere (not just home network), get [Personal Remote](https://clawde.io/#pricing)

## Verify Everything Works

```bash
clawd doctor
```

Expected output:

```text
  ✓ Port 4300 available      port 4300 is free
  ✓ claude CLI installed     claude 1.x.x
  ✓ claude CLI authenticated  logged in
  ✓ SQLite DB accessible     ~/.local/share/clawd/clawd.db
  ✓ Disk space               45GB free
  ✓ Relay reachable          api.clawde.io reachable

All checks passed.
```

## Next Steps

- [[Features/Projects]] — organize your repos into projects
- [[Features/Remote-Access]] — connect from your phone or another machine
- [[Configuration]] — customize the daemon
- [[Daemon-Reference]] — JSON-RPC API for building clients
