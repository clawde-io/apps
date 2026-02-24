# Changelog

All notable changes to ClawDE are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

No unreleased changes yet.

---

## [0.1.0] — 2026-02-23

First public release. Binaries available for macOS (Apple Silicon + Intel), Linux x86\_64, and Windows x86\_64.

### Added

#### Daemon (`clawd`)

- JSON-RPC 2.0 over WebSocket server on `localhost:4300`
- Session management: create, list, get, delete, pause, resume, cancel
- Message streaming from Claude Code subprocess (`claude` CLI)
- Tool call lifecycle: pending → approve/reject → done
- Repo integration: open/close repos, watch file changes, git status/diff
- Project model: group repos into named projects (RPCs: `project.*`)
- Device pairing: QR code + PIN flow for remote mobile access (RPCs: `device.*`)
- HTTP health endpoint: `GET http://127.0.0.1:4300/health`
- Auth token stored at platform-standard path (mode 0600)
- `clawd start` / `clawd stop` / `clawd status` / `clawd token show` / `clawd token qr`
- SQLite WAL-mode database with versioned migrations
- mDNS LAN discovery (advertises `_clawd._tcp` service with port in TXT record)
- Configurable bind address (`--bind` flag, `CLAWD_BIND` env var, `config.toml`)
- Resource governor: RAM pressure monitoring, session eviction
- Structured TOML config (`config.toml` in platform data dir)
- SPDX license compliance (MIT headers, `NOTICE` file)

#### Dart packages

- `clawd_proto`: all protocol types (Session, Message, ToolCall, push events)
- `clawd_client`: typed WebSocket/JSON-RPC client with reconnection backoff
- `clawd_core`: Riverpod providers (daemon connection, session list, message list, tool calls)
- `clawd_ui`: shared Flutter widgets (ChatBubble, ToolCallCard, MessageInput, ConnectionBanner, ProviderBadge)

#### Flutter apps

Desktop app (macOS / Windows / Linux) and mobile app (iOS / Android) with session list,
chat view, tool approval flow, and settings screen. Platform runners and full UI polish
ship in v0.2.0.

#### Distribution

- `curl -fsSL https://clawde.io/install.sh | bash` — one-line installer (macOS + Linux)
- Homebrew tap: `brew tap clawde-io/clawde && brew install clawd`
- GitHub Releases with SHA256 checksums for all 4 platform binaries

#### Testing

- 264 Rust tests (unit + integration: session recovery, health endpoint)
- 153 Flutter tests (clawd\_core, desktop widget, mobile widget, proto/client/ui)
- CI: GitHub Actions on push/PR (cargo clippy, rustfmt, Dart analyze, dart test, flutter test)

---

[Unreleased]: <https://github.com/clawde-io/apps/compare/v0.1.0...HEAD>
[0.1.0]: <https://github.com/clawde-io/apps/releases/tag/v0.1.0>
