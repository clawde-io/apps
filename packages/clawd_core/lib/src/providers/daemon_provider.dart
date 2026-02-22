import 'dart:async';
import 'dart:developer' as dev;
import 'dart:math';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_client/clawd_client.dart';
import 'settings_provider.dart';

/// The connection state of the local clawd daemon.
enum DaemonStatus { disconnected, connecting, connected, error }

/// Runtime information returned by the `daemon.status` RPC.
class DaemonInfo {
  final String version;
  final int uptime; // seconds
  final int activeSessions;
  final int port;

  const DaemonInfo({
    required this.version,
    required this.uptime,
    required this.activeSessions,
    required this.port,
  });

  factory DaemonInfo.fromJson(Map<String, dynamic> json) => DaemonInfo(
        version: json['version'] as String,
        uptime: json['uptime'] as int,
        activeSessions: json['active_sessions'] as int,
        port: json['port'] as int,
      );
}

class DaemonState {
  final DaemonStatus status;
  final String? errorMessage;
  final DaemonInfo? daemonInfo;
  /// Number of reconnect attempts made since last disconnect. 0 = first attempt.
  final int reconnectAttempt;

  const DaemonState({
    this.status = DaemonStatus.disconnected,
    this.errorMessage,
    this.daemonInfo,
    this.reconnectAttempt = 0,
  });

  bool get isConnected => status == DaemonStatus.connected;

  DaemonState copyWith({
    DaemonStatus? status,
    String? errorMessage,
    DaemonInfo? daemonInfo,
    int? reconnectAttempt,
  }) =>
      DaemonState(
        status: status ?? this.status,
        errorMessage: errorMessage ?? this.errorMessage,
        daemonInfo: daemonInfo ?? this.daemonInfo,
        reconnectAttempt: reconnectAttempt ?? this.reconnectAttempt,
      );
}

/// Manages the singleton ClawdClient and its connection lifecycle.
/// Both desktop and mobile share this provider via ProviderScope.
class DaemonNotifier extends Notifier<DaemonState> {
  late ClawdClient _client;
  int _reconnectAttempt = 0;
  Timer? _reconnectTimer;
  bool _disposed = false;

  static const int _maxReconnectAttempts = 8;
  static const Duration _baseDelay = Duration(seconds: 2);
  static const Duration _maxDelay = Duration(seconds: 60);

  @override
  DaemonState build() {
    // Read initial URL without subscribing — build() stays stable.
    final url = ref.read(settingsProvider).valueOrNull?.daemonUrl
        ?? 'ws://127.0.0.1:4300';
    _client = ClawdClient(url: url);

    ref.onDispose(() {
      _disposed = true;
      _reconnectTimer?.cancel();
      _client.disconnect();
    });

    // Reconnect with new URL when the daemon URL setting changes.
    ref.listen(settingsProvider, (prev, next) {
      final newUrl = next.valueOrNull?.daemonUrl;
      final oldUrl = prev?.valueOrNull?.daemonUrl;
      if (newUrl != null && newUrl != oldUrl) {
        _switchUrl(newUrl);
      }
    });

    // Auto-connect on first use.
    _connect();
    return const DaemonState(status: DaemonStatus.connecting);
  }

  /// Switch to a new daemon URL — disconnect old client, connect to new one.
  Future<void> _switchUrl(String newUrl) async {
    if (_disposed) return;
    _reconnectTimer?.cancel();
    _reconnectAttempt = 0;
    _client.disconnect();
    _client = ClawdClient(url: newUrl);
    await _connect();
  }

  Future<void> _connect() async {
    if (_disposed) return;
    _reconnectTimer?.cancel();
    state = const DaemonState(status: DaemonStatus.connecting);
    try {
      await _client.connect();
      if (_disposed) return;
      _reconnectAttempt = 0;
      state = const DaemonState(status: DaemonStatus.connected);
      _listenForPushEvents();
      // Best-effort: fetch daemon info after connecting.
      await refreshStatus();
    } on ClawdDisconnectedError catch (e) {
      if (_disposed) return;
      dev.log('Connect failed (disconnected): $e', name: 'clawd_core');
      state = DaemonState(
          status: DaemonStatus.error, errorMessage: e.toString());
      _scheduleReconnect();
    } catch (e) {
      if (_disposed) return;
      dev.log('Connect failed: $e', name: 'clawd_core');
      state = DaemonState(
          status: DaemonStatus.error, errorMessage: e.toString());
      _scheduleReconnect();
    }
  }

  void _scheduleReconnect() {
    if (_disposed || _reconnectAttempt >= _maxReconnectAttempts) return;
    final delay = _backoffDelay(_reconnectAttempt);
    dev.log(
      'Reconnect attempt ${_reconnectAttempt + 1} in ${delay.inSeconds}s',
      name: 'clawd_core',
    );
    state = state.copyWith(
      status: DaemonStatus.connecting,
      reconnectAttempt: _reconnectAttempt + 1,
    );
    _reconnectAttempt++;
    _reconnectTimer = Timer(delay, _connect);
  }

  Duration _backoffDelay(int attempt) {
    final ms = _baseDelay.inMilliseconds * pow(2, attempt).toInt();
    final jitter = Random().nextInt(1000); // up to 1s jitter
    return Duration(
        milliseconds: min(ms + jitter, _maxDelay.inMilliseconds));
  }

  /// Reconnect immediately (e.g. user tap or app foreground).
  Future<void> reconnect() {
    _reconnectAttempt = 0;
    return _connect();
  }

  /// Fetch daemon runtime info (version, uptime, active sessions, port).
  /// Graceful: failures set daemonInfo to null without disconnecting.
  Future<void> refreshStatus() async {
    try {
      final result = await _client.call<Map<String, dynamic>>('daemon.status');
      if (_disposed) return;
      state = state.copyWith(daemonInfo: DaemonInfo.fromJson(result));
    } catch (_) {
      if (_disposed) return;
      state = state.copyWith(daemonInfo: null);
    }
  }

  void _listenForPushEvents() {
    _client.pushEvents.listen(
      (event) {
        // Push events are handled by individual providers via ref.listen.
        // This stream is exposed via daemonPushEventsProvider.
      },
      onError: (e) {
        if (_disposed) return;
        dev.log('Push stream error: $e', name: 'clawd_core');
        state = const DaemonState(status: DaemonStatus.disconnected);
        _scheduleReconnect();
      },
      onDone: () {
        if (_disposed) return;
        state = const DaemonState(status: DaemonStatus.disconnected);
        _scheduleReconnect();
      },
    );
  }

  /// Exposes the underlying client so other providers can make RPC calls.
  ClawdClient get client => _client;
}

final daemonProvider = NotifierProvider<DaemonNotifier, DaemonState>(
  DaemonNotifier.new,
);

/// Exposes the raw push-event stream from the daemon for providers that need
/// to react to server-pushed notifications (e.g. new messages, tool calls).
final daemonPushEventsProvider = StreamProvider<Map<String, dynamic>>((ref) {
  final notifier = ref.watch(daemonProvider.notifier);
  return notifier.client.pushEvents;
});
