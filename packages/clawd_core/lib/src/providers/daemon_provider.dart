import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_client/clawd_client.dart';

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

  const DaemonState({
    this.status = DaemonStatus.disconnected,
    this.errorMessage,
    this.daemonInfo,
  });

  bool get isConnected => status == DaemonStatus.connected;

  DaemonState copyWith({
    DaemonStatus? status,
    String? errorMessage,
    DaemonInfo? daemonInfo,
  }) =>
      DaemonState(
        status: status ?? this.status,
        errorMessage: errorMessage ?? this.errorMessage,
        daemonInfo: daemonInfo ?? this.daemonInfo,
      );
}

/// Manages the singleton ClawdClient and its connection lifecycle.
/// Both desktop and mobile share this provider via ProviderScope.
class DaemonNotifier extends Notifier<DaemonState> {
  late final ClawdClient _client;

  @override
  DaemonState build() {
    _client = ClawdClient();
    ref.onDispose(_client.disconnect);
    // Auto-connect on first use.
    _connect();
    return const DaemonState(status: DaemonStatus.connecting);
  }

  Future<void> _connect() async {
    state = const DaemonState(status: DaemonStatus.connecting);
    try {
      await _client.connect();
      state = const DaemonState(status: DaemonStatus.connected);
      _listenForPushEvents();
      // Best-effort: fetch daemon info after connecting.
      await refreshStatus();
    } on ClawdDisconnectedError catch (e) {
      state = DaemonState(status: DaemonStatus.error, errorMessage: e.toString());
    } catch (e) {
      state = DaemonState(status: DaemonStatus.error, errorMessage: e.toString());
    }
  }

  /// Reconnect (e.g. after app foreground or explicit user retry).
  Future<void> reconnect() => _connect();

  /// Fetch daemon runtime info (version, uptime, active sessions, port).
  /// Graceful: failures set daemonInfo to null without disconnecting.
  Future<void> refreshStatus() async {
    try {
      final result = await _client.call<Map<String, dynamic>>('daemon.status');
      state = state.copyWith(daemonInfo: DaemonInfo.fromJson(result));
    } catch (_) {
      state = state.copyWith(daemonInfo: null);
    }
  }

  void _listenForPushEvents() {
    _client.pushEvents.listen(
      (event) {
        // Push events are handled by individual providers via ref.listen.
        // This stream is exposed via pushEventsProvider.
      },
      onError: (_) {
        state = const DaemonState(status: DaemonStatus.disconnected);
        // Back-off reconnect handled by individual apps.
      },
      onDone: () {
        state = const DaemonState(status: DaemonStatus.disconnected);
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
