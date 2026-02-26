# Contributing to ClawDE

ClawDE welcomes contributions of all kinds — bug reports, feature requests, documentation improvements, and code.

## Development Setup

### Prerequisites

- Rust 1.82+ (`rustup update stable`)
- Flutter 3.27+ (`flutter upgrade`)
- Dart 3.7+

### Clone and build

```bash
git clone https://github.com/clawde-io/apps
cd apps

# Build the daemon
cd daemon
cargo build

# Run tests
cargo test

# Run the Flutter apps (desktop)
cd ../desktop
flutter pub get
flutter run -d macos
```

### Daemon development

```bash
cd daemon
cargo run -- --port 4300 --data-dir /tmp/clawd-dev
```

Logs go to stdout. Set `RUST_LOG=debug` for verbose output.

## Pull Request Process

1. Fork the repo and create a branch: `git checkout -b feat/my-feature`
2. Write your code. Follow the existing style — `cargo clippy` must pass with no warnings
3. Add tests for new functionality
4. Run `cargo test && flutter test`
5. Submit a PR with a clear description of what changed and why
6. A maintainer will review within a few days

## Code Style

- **Rust:** `cargo fmt` before committing. No `unwrap()` in production paths — use `?` operator
- **Dart/Flutter:** `dart format .` before committing. Follow existing provider patterns
- **Commits:** Conventional commits preferred: `feat:`, `fix:`, `docs:`, `refactor:`

## Reporting Bugs

Open a [GitHub Issue](https://github.com/clawde-io/apps/issues) with:
- ClawDE version (`clawd --version`)
- OS and version
- Steps to reproduce
- Expected vs actual behavior

## Security Issues

**Do not open public issues for security vulnerabilities.** See [SECURITY.md](SECURITY.md).

## Questions

Join the [ClawDE Discord](https://discord.gg/clawde) — `#dev` channel for contributors.
