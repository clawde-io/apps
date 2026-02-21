# Changelog

All notable changes to ClawDE are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added
- Initial project scaffold: Rust daemon (`clawd`), Flutter desktop app, Flutter mobile app
- Dart packages: `clawd_proto`, `clawd_client`, `clawd_core`, `clawd_ui`
- JSON-RPC 2.0 over WebSocket IPC protocol (17 methods, 7 push events)
- Session management, message streaming, tool call approval flow
- Multi-provider support: Claude Code, Codex, Cursor, Aider
- Shared Riverpod state management (`clawd_core`)
- Shared Flutter widget library (`clawd_ui`)
- CI/CD: GitHub Actions for Dart analysis/tests and Rust clippy/fmt/tests

---

*[Unreleased]: https://github.com/clawde-io/apps/compare/HEAD*
