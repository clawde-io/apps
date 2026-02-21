# ClawDE

**Your IDE. Your Rules.**

ClawDE is an AI-first developer environment that runs on your machine. One always-on local daemon (`clawd`) manages your AI sessions, tracks your code, and keeps every agent in sync. Flutter apps on desktop and mobile connect to it over a local WebSocket.

## Why ClawDE?

| Problem | ClawDE's answer |
| --- | --- |
| **Drift** — AI agents forget context between sessions | `clawd` persists every session, message, and repo state in SQLite |
| **Gaps** — switching tools resets your AI's understanding | Continuous daemon means no cold starts |
| **Hallucinations** — agents invent things that aren't there | Daemon validates against the real filesystem and git history |

## Key features

- Works with **Claude Code, Codex, Cursor, and Aider** — one interface for all
- **Desktop app** for macOS, Windows, and Linux
- **Mobile companion** for iOS and Android — monitor and reply from anywhere
- **Free forever** for local use — no subscription required to run on your own machine
- **Open source** — Rust daemon + Flutter apps, MIT licensed

## Quick links

- [[Getting-Started]] — install and run in under 5 minutes
- [[Architecture]] — how the daemon, apps, and packages fit together
- [[Features]] — full feature list with status
- [[Contributing]] — how to contribute code
- [[Changelog]] — version history
- [[FAQ]] — common questions

## Distribution

| Mode | Who hosts | Price |
| --- | --- | --- |
| **Self-hosted** | You, on your machine | Free or $9.99/year (remote access) |
| **ClawDE Cloud** | Us, on Hetzner | $20–$200/month |

The open-source code in this repo covers the **self-hosted** mode entirely.
