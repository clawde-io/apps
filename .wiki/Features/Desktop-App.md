# Desktop App

The Tauri v2 desktop IDE with Monaco editor, file tree, chat UI, and full access to the local daemon.

## Overview

The ClawDE desktop app is a lightweight IDE purpose-built for AI-assisted development. Built with Tauri v2, it uses the OS native WebView for the UI and Rust for the backend, resulting in a small binary with native performance.

## Two Modes

### File Mode

Open a single file for focused editing with an AI chat panel. Ideal for quick edits, code review, or asking questions about a specific file.

- Sublime-style single-file editor
- Right-side chat panel focused on the file's context
- Minimal scanning, fast startup

### Project Mode

Open a folder for full IDE functionality. Left-rail navigation, multi-file editing, session management, and all ClawDE features.

- Explorer, Search, Git, Chat, Sessions, Packs, Settings panels
- Monaco-based multi-tab editor
- Multi-session management (view and manage concurrent AI sessions)

## Technology

| Component | Technology |
| --- | --- |
| Framework | Tauri v2 |
| UI | React + TypeScript + Tailwind CSS |
| Editor | Monaco (VS Code engine) |
| Backend | Rust (connects to daemon) |
| Rendering | OS WebView (WKWebView, WebView2, WebKitGTK) |

## Features

- Full Monaco editor with syntax highlighting, IntelliSense, and minimap
- File tree with git status indicators
- Integrated terminal
- Chat UI with thinking indicators, tool cards, and diff review
- Session list showing all active, paused, and completed sessions
- System tray icon with quick actions
- Auto-update via Tauri's built-in updater
