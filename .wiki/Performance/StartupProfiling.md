# Startup Profiling

**Sprint CC · PERF.2**

This document describes how to profile ClawDE desktop startup time and the
techniques used to keep cold-start under 2 seconds on a mid-range machine.

---

## What "startup" means

ClawDE startup covers three phases:

| Phase | From | To |
| --- | --- | --- |
| **Dart VM init** | `main()` called | `WidgetsFlutterBinding.ensureInitialized()` done |
| **App boot** | Flutter binding ready | First frame painted (native window visible) |
| **Daemon ready** | Window shown | `DaemonStatus.connected` first received |

---

## Profiling with Flutter DevTools

### 1. Performance overlay (quick)

Run in profile mode and enable the overlay:

```bash
flutter run --profile -d macos
```

In DevTools → **Performance** tab → enable **Show performance overlay**.

The top bar shows UI thread frame time; bottom shows raster thread. Both
should stay under 16 ms (60 fps) during startup.

### 2. Timeline trace (detailed)

```bash
flutter run --profile -d macos --trace-startup
```

This writes a `start_up_info.json` to the project root with:
- `engineEnterTimestampMicros`
- `timeToFirstFrameMicros`
- `timeToFrameworkInitMicros`
- `timeToFirstFrameRasterizedMicros`

Open the file in DevTools → **Performance** → **Import trace**.

### 3. App size analysis

```bash
flutter build macos --analyze-size
```

Large tree-shaken packages inflate startup time. The output shows top
contributors by size.

---

## Known hot paths

| Location | Cost | Notes |
| --- | --- | --- |
| `DaemonManager.ensureRunning()` | ~300–800 ms | Spawns `clawd` subprocess; blocked by process exec |
| `UpdaterService.init()` | ~50 ms | Reads `pubspec.yaml` and compares with GitHub Releases |
| `TrayService.init()` | ~20 ms | Loads tray icon PNG from assets |
| First `DaemonNotifier` reconnect | ~100–200 ms | WebSocket TCP handshake to localhost |
| `sessionListProvider` initial fetch | ~50–100 ms | SQLite query through daemon RPC |

---

## Optimization techniques in use

### Async parallel init

`main()` runs `DaemonManager.ensureRunning()`, `TrayService.init()`, and
`UpdaterService.init()` sequentially. If cold-start exceeds 1.5 s, parallelise
these using `Future.wait`:

```dart
await Future.wait([
  DaemonManager.instance.ensureRunning(),
  TrayService.instance.init(...),
  UpdaterService.instance.init(),
]);
```

**Not done yet** — preserved for Sprint PERF.3 once baseline is measured.

### Lazy provider initialization

All Riverpod providers are `.autoDispose` — they only initialize when the first
widget subscribes. Providers not on the initial route (evals, ghost diff, etc.)
do not run at startup.

### Widget tree depth

`AppShell` uses `IndexedStack` for navigation tabs. Non-visible tabs are kept
alive but do not trigger provider initialisation until first visit.

---

## Target metrics

| Metric | Target | Measurement |
| --- | --- | --- |
| Time to first frame | < 500 ms | `timeToFirstFrameMicros` in trace |
| Time to daemon connected | < 2 000 ms | `DaemonStatus.connected` event delta from `main()` |
| Memory at startup | < 120 MB | Activity Monitor → Real Memory |

---

## Running the benchmark

```bash
# Build profile binary
flutter build macos --profile

# Launch and capture trace
/Applications/ClawDE.app/Contents/MacOS/ClawDE --trace-startup \
  2>&1 | tee /tmp/clawd-startup.log

# Parse first-frame time
grep "timeToFirstFrameMicros" start_up_info.json
```

Divide `timeToFirstFrameMicros` by 1 000 to get milliseconds.
