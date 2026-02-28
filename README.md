# ClawDE

The host-first AI development environment.

One daemon. Every provider. Any device.

## Documentation

See the [Wiki](https://github.com/clawde-io/apps/wiki) for full documentation:

- [Getting Started](https://github.com/clawde-io/apps/wiki/Getting-Started)
- [Architecture](https://github.com/clawde-io/apps/wiki/Architecture)
- [Features](https://github.com/clawde-io/apps/wiki/Features)
- [Contributing](https://github.com/clawde-io/apps/wiki/Contributing)
- [Changelog](https://github.com/clawde-io/apps/wiki/Changelog)

## Quick Start

```bash
# Clone the repo
git clone https://github.com/clawde-io/apps.git
cd apps

# Bootstrap Dart/Flutter workspace
cd apps
dart pub global activate melos
melos bootstrap

# Build and run the daemon
cd daemon && cargo build --release

# Run the desktop app
cd ../desktop && flutter run
```

See [Getting Started](https://github.com/clawde-io/apps/wiki/Getting-Started) for full setup instructions.

## Structure

```text
apps/         # All application code
  daemon/     # clawd — Rust/Tokio daemon
  desktop/    # Flutter desktop app (macOS/Windows/Linux)
  mobile/     # Flutter mobile app (iOS/Android)
  packages/   # Shared Dart packages
web/          # Website (clawde.io)
.github/      # CI/CD workflows, wiki source, brand assets
```

## License

[MIT](LICENSE)
