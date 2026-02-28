# LSP Integration

ClawDE integrates with Language Server Protocol (LSP) servers to enrich AI context with real-time type information, diagnostics, and symbol resolution.

## What this enables

- AI sees current compiler errors before suggesting a fix
- Symbol hover info and type signatures are injected into prompts automatically
- "Fix all errors in this file" tasks use live diagnostic data, not stale context
- Go-to-definition results help the AI understand cross-file dependencies

## How it works

The daemon spawns or connects to the LSP server for the active repo's language. It subscribes to `textDocument/publishDiagnostics` and `workspace/symbol` notifications. This context is merged into the system prompt for each session turn.

Supported LSP servers (v0.3.0 target):

| Language | LSP server |
| --- | --- |
| Rust | rust-analyzer |
| TypeScript / JavaScript | typescript-language-server |
| Python | pylsp / pyright |
| Go | gopls |
| Dart | Dart Analysis Server |

## Requirements

The LSP server must be installed and available in `PATH`. ClawDE does not bundle LSP servers.

## Status

Planned for Sprint II. Requires LSP client implementation in the daemon.

## Related

- [Repo Intelligence](Repo-Intelligence.md)
- [Daemon Reference](../Daemon-Reference.md)
