import 'dart:async';
import 'dart:developer' as dev;
import 'dart:io';

/// Manages the lifecycle of the bundled `clawd` daemon process.
///
/// On app startup, [ensureRunning] checks if a daemon is already listening on
/// port 4300 (e.g. dev mode with a manually-started daemon). If not, it
/// locates the bundled `clawd` binary next to the Flutter executable, spawns
/// it, and waits up to 5 s for the auth token file to appear.
///
/// [shutdown] sends SIGTERM with a 3-second grace period, then SIGKILL.
class DaemonManager {
  DaemonManager._();
  static final DaemonManager instance = DaemonManager._();

  Process? _process;
  String? _tokenOverride;

  static const int _port = 4300;

  /// The auth token read from the daemon's token file after spawning.
  /// Desktop app injects this into [bootstrapTokenProvider] so the Riverpod
  /// layer does not need to race against the file appearing on disk.
  String? get tokenOverride => _tokenOverride;

  /// Ensure the daemon is running.  Safe to call multiple times — if the
  /// daemon is already listening, just reads the existing token and returns.
  Future<void> ensureRunning() async {
    if (await _tryPing()) {
      dev.log('daemon already running on port $_port', name: 'DaemonManager');
      _tokenOverride = _readTokenSync();
      return;
    }

    final binary = _locateBinary();
    if (binary == null || !binary.existsSync()) {
      dev.log(
        'clawd binary not found next to executable — dev mode, skip spawn',
        name: 'DaemonManager',
      );
      return;
    }

    dev.log('spawning clawd: ${binary.path}', name: 'DaemonManager');
    _process = await Process.start(
      binary.path,
      ['serve'],
      mode: ProcessStartMode.detachedWithStdio,
    );
    // Drain stdio so the process is not blocked by a full pipe buffer.
    unawaited(_process!.stdout.drain<List<int>>());
    unawaited(_process!.stderr.drain<List<int>>());

    _tokenOverride = await _pollForToken(const Duration(seconds: 5));
    if (_tokenOverride == null) {
      dev.log('clawd token did not appear within 5 s', name: 'DaemonManager');
    } else {
      dev.log('clawd token ready', name: 'DaemonManager');
    }
  }

  /// Gracefully stop the daemon process managed by this instance.
  /// No-op if the daemon was already running when the app started (i.e. we
  /// did not spawn it).
  Future<void> shutdown() async {
    if (_process == null) return;
    dev.log('shutting down clawd', name: 'DaemonManager');
    _process!.kill(ProcessSignal.sigterm);
    await _process!.exitCode.timeout(
      const Duration(seconds: 3),
      onTimeout: () {
        _process!.kill(ProcessSignal.sigkill);
        return -1;
      },
    );
    _process = null;
  }

  // ── Private helpers ───────────────────────────────────────────────────────

  /// Returns true if something is already accepting connections on port 4300.
  Future<bool> _tryPing() async {
    try {
      final socket = await Socket.connect(
        '127.0.0.1',
        _port,
        timeout: const Duration(milliseconds: 300),
      );
      socket.destroy();
      return true;
    } catch (_) {
      return false;
    }
  }

  /// Locate the `clawd` (or `clawd.exe`) binary next to the Flutter executable.
  File? _locateBinary() {
    try {
      final dir = File(Platform.resolvedExecutable).parent;
      final name = Platform.isWindows ? 'clawd.exe' : 'clawd';
      return File('${dir.path}/$name');
    } catch (_) {
      return null;
    }
  }

  /// Read the auth token from the platform-appropriate data directory.
  String? _readTokenSync() {
    try {
      final path = _tokenFilePath();
      if (path == null) return null;
      final file = File(path);
      if (!file.existsSync()) return null;
      final token = file.readAsStringSync().trim();
      return token.isEmpty ? null : token;
    } catch (_) {
      return null;
    }
  }

  /// Poll every 200 ms until the token file appears or [timeout] elapses.
  Future<String?> _pollForToken(Duration timeout) async {
    final deadline = DateTime.now().add(timeout);
    while (DateTime.now().isBefore(deadline)) {
      final token = _readTokenSync();
      if (token != null) return token;
      await Future<void>.delayed(const Duration(milliseconds: 200));
    }
    return null;
  }

  /// Platform-appropriate path for the daemon's auth token file.
  /// Must match the path computed by `clawd/src/config/mod.rs`.
  static String? _tokenFilePath() {
    if (Platform.isMacOS) {
      final home = Platform.environment['HOME'];
      return home != null
          ? '$home/Library/Application Support/clawd/auth_token'
          : null;
    }
    if (Platform.isLinux) {
      final xdg = Platform.environment['XDG_DATA_HOME'];
      if (xdg != null) return '$xdg/clawd/auth_token';
      final home = Platform.environment['HOME'];
      return home != null ? '$home/.local/share/clawd/auth_token' : null;
    }
    if (Platform.isWindows) {
      final appdata = Platform.environment['APPDATA'];
      return appdata != null ? '$appdata\\clawd\\auth_token' : null;
    }
    return null;
  }
}
