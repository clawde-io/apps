# Getting Started

## Prerequisites

| Requirement | Version |
| --- | --- |
| macOS / Linux / Windows | Any modern version |
| [Rust](https://rustup.rs) | 1.78+ (for building from source) |
| [Flutter](https://flutter.dev/docs/get-started/install) | 3.24+ (for building the apps) |
| Any AI provider | Claude Code, Codex, Cursor, or Aider |

## Option A — Download a release (coming soon)

Pre-built binaries will be available on the [Releases](https://github.com/clawde-io/apps/releases) page. Download `clawd` for your platform, add it to your PATH, and start the daemon.

## Option B — Build from source

### 1. Clone the repo

```bash
git clone https://github.com/clawde-io/apps.git
cd apps
```

### 2. Build the daemon

```bash
cd daemon
cargo build --release
# Binary is at daemon/target/release/clawd
```

Add it to your PATH:

```bash
# macOS / Linux
export PATH="$PATH:$(pwd)/target/release"
```

### 3. Start the daemon

```bash
clawd start
# Daemon listens on ws://127.0.0.1:4300
```

### 4. Build and run the desktop app

```bash
# Install melos
dart pub global activate melos

# Install all Dart dependencies
melos bootstrap

# Run on macOS
melos run dev
```

## First session

1. Open the ClawDE desktop app
2. Click **New session** and choose a repo
3. ClawDE detects your installed AI providers (Claude Code, Codex, etc.)
4. Start chatting — the daemon streams responses back in real time

## Remote access (Personal Remote tier)

To access your sessions from anywhere (including the mobile app over the internet):

1. Sign in at [clawde.io](https://clawde.io) and subscribe to **Personal Remote** ($9.99/year)
2. Run `clawd relay start` — the daemon establishes an outbound mTLS tunnel to `api.clawde.io`
3. Open the mobile app and pair with the relay QR code

Your machine stays the host. We just route the traffic.
