# Desktop App

The Flutter desktop app with chat-first UI, file tree, and full access to the local daemon. Available for macOS, Windows, and Linux.

## Overview

The ClawDE desktop app is a chat-first client for AI-assisted development. Built with Flutter (Dart), it is a native app for macOS, Windows, and Linux. It connects to the local `clawd` daemon over WebSocket and acts as a window into your development environment — the daemon does all the heavy lifting.

## Two Modes

### File Mode

Open a single file for focused editing with an AI chat panel. Ideal for quick edits, code review, or asking questions about a specific file.

- Single-file view with chat panel
- Right-side chat focused on the file's context
- Minimal scanning, fast startup

### Project Mode

Open a folder for full IDE functionality. Left-rail navigation, multi-file editing, session management, and all ClawDE features.

- Explorer, Search, Git, Chat, Sessions, Packs, Settings panels
- Flutter-native code editor with multi-tab support
- Multi-session management (view and manage concurrent AI sessions)

## Technology

| Component | Technology |
| --- | --- |
| Framework | Flutter (Dart) |
| Platforms | macOS, Windows, Linux |
| UI | Flutter widgets + Material / custom theme |
| Daemon connection | WebSocket (localhost:4300, JSON-RPC 2.0) |
| Auto-updater | Sparkle (macOS), WinSparkle (Windows) |

## Features

- Chat-first experience — conversation is the primary view
- Navigation rail: Chat, Files, Git, Search, Sessions, Packs, Topology, Settings
- File tree with git status indicators
- Flutter-native code editor with syntax highlighting and diff view
- Chat UI with thinking indicators, tool cards, and diff review
- Session list showing all active, paused, and completed sessions
- System tray icon with quick actions
- Command palette (Cmd+Shift+P) and quick file open (Cmd+P)
- Auto-update via Sparkle (macOS) and WinSparkle (Windows)
