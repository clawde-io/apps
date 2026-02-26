# Plugin System

> Sprint FF — Extend ClawDE with native and WASM plugins.

## Overview

ClawDE's plugin system lets you extend the daemon with custom logic that runs alongside every AI session. Plugins receive lifecycle events (session start/end, tool calls, messages) and can emit push events to connected clients.

Two runtimes are supported:

| Runtime | Extension | Use when |
| --- | --- | --- |
| **Dylib** | `.dylib` / `.so` / `.dll` | Maximum performance, system access |
| **WASM** | `.wasm` | Sandboxed, portable, safe for third-party plugins |

## Quick Start

### Install a plugin

```bash
clawd pack install <plugin-name>
```

Plugins are distributed as pack archives. After installation, the plugin appears in the Plugin Manager and is enabled automatically.

### Scaffold a new plugin

```bash
# Native Rust dylib plugin
clawd plugin scaffold --type dylib --name my-plugin

# WASM plugin (Rust→WASM)
clawd plugin scaffold --type wasm --name my-wasm-plugin
```

### Enable / disable

```bash
clawd plugin list
clawd plugin enable my-plugin
clawd plugin disable my-plugin
```

Or use the **Plugin Manager** page in the desktop app (Settings → Plugins).

## Plugin Manifest

Every plugin must include a `clawd-plugin.json` at its pack root:

```json
{
  "name": "hello-clawd",
  "version": "1.0.0",
  "description": "Greets each new session",
  "author": "you",
  "runtime": "dylib",
  "entry": "libhello_clawd.dylib",
  "capabilities": [],
  "signature": ""
}
```

### Capability grants

| Capability | What it allows |
| --- | --- |
| `fs.read` | Read files in the project directory |
| `fs.write` | Write files in the project directory |
| `network.relay` | Send events to connected clients |
| `daemon.rpc` | Call daemon RPC methods |

Users are prompted to approve capability grants on first install. The daemon enforces them on every host call.

## Signing

Official plugins distributed through the ClawDE registry are signed with the registry's Ed25519 private key. Self-signed plugins are allowed but the user is warned.

```bash
# Generate a signing keypair
clawd plugin genkey

# Sign a plugin binary
clawd plugin sign libmy_plugin.dylib --key private_key.hex

# Verify a signature
clawd plugin verify libmy_plugin.dylib --pubkey public_key.hex
```

## RPC Methods

| Method | Description |
| --- | --- |
| `plugin.list` | List all installed plugins |
| `plugin.enable` | Enable a plugin by name |
| `plugin.disable` | Disable a plugin by name |
| `plugin.info` | Get detail for a single plugin |

## Example Plugins

| Plugin | Runtime | What it does |
| --- | --- | --- |
| `hello-clawd` | dylib | Logs "Hello!" on session start |
| `auto-test` | WASM | Runs tests on every `task_done` event |

Source: `examples/plugins/` in the `apps` repo.

## ABI Reference

See [Plugin ABI](../Developing/PluginABI.md) for the full C ABI specification and stability guarantee.
