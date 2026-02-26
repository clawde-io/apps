# Code Editor

ClawDE's desktop app includes a full code editor powered by CodeMirror 6 running inside a Flutter WebView.

## Features

| Feature | Description |
| --- | --- |
| Syntax highlighting | 30+ languages via `@codemirror/lang-*` |
| Multi-tab editing | Open multiple files as tabs with dirty indicators |
| Split pane | Horizontal or vertical split — each pane is independent |
| Diff viewer | Side-by-side diff using `@codemirror/merge` |
| Session gutter | Lines read/written by AI are highlighted in the gutter |
| Ghost text | Inline AI completion suggestions (Tab to accept) |
| File tree | Left sidebar file browser with context menu |
| Go-to-definition | Cmd+Click via `lsp.definition` RPC |
| Global search | Cmd+K opens the cross-session full-text search overlay |

## Architecture

The editor uses `webview_flutter` to embed a CodeMirror 6 instance loaded from `assets/editor/editor.html`. Communication between Flutter and the JavaScript editor is bidirectional through a named JavaScript channel (`ClawdBridge`).

```
Flutter ←→ JsBridge ←→ ClawdBridge (JS channel) ←→ CodeMirror 6
```

### File open protocol (ED.2)

```dart
bridge.sendOpenFile(path: 'src/main.rs', content: '...', language: 'rust');
```

On the JS side, `window.clawdOpen({ type: 'open', path, content, language })` is called, which replaces the editor content and sets the syntax mode.

### Save protocol (ED.3)

CodeMirror fires `onChange` → 500 ms debounce → posts `{ type: 'change', content }` to `ClawdBridge` → Flutter calls `fs.write` RPC with the new content.

## Keyboard Shortcuts

| Shortcut | Action |
| --- | --- |
| `Cmd+S` | Save current file |
| `Cmd+W` | Close current tab |
| `Cmd+Shift+[` | Switch to previous tab |
| `Cmd+Shift+]` | Switch to next tab |
| `Cmd+\` | Toggle split pane |
| `Cmd+K` | Open global session search |
| `Tab` | Accept inline completion suggestion |
| `Escape` | Dismiss inline completion suggestion |

## Editor Theme

The `clawdEditorTheme` in `assets/editor/editor_theme.js` applies the ClawDE brand palette:

- Background: `#0f0f14`
- Cursor: `#dc2626` (brand red)
- Selection: `#7f1d1d`
- Active line: `#1a1a1f`
- Amber accent (operators): `#ff784e`

## Session Gutter

The session-aware gutter marks lines that were read or written by an AI session:

- **Red strip** — written by AI
- **Blue strip** — read by AI
- **Amber strip** — both read and written

Hover over a strip to see which session touched the line.

## RPC Methods Used

The code editor does not add new RPC methods. It uses existing:

- `fs.list` — file tree directory listing
- `fs.write` — save file changes
- `fs.read` — initial file load
- `lsp.definition` — go-to-definition
- `repo.searchSymbol` — fallback symbol search
- `completion.complete` — inline AI completions
- `session.search` — global search overlay
