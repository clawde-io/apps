import 'dart:async';
import 'dart:convert';

import 'package:clawd_proto/clawd_proto.dart';
import 'package:web_socket_channel/web_socket_channel.dart';

import 'exceptions.dart';

/// Default clawd daemon port.
const int kClawdPort = 4300;

/// Default timeout for RPC calls. Long-running operations (e.g. session.create,
/// session.sendMessage) inherit this; callers can override per-call if needed.
const Duration kDefaultCallTimeout = Duration(seconds: 30);

/// JSON-RPC 2.0 WebSocket client for the clawd daemon.
///
/// Usage:
/// ```dart
/// final client = ClawdClient();
/// await client.connect();
/// final session = Session.fromJson(await client.call('session.create', {...}));
/// ```
class ClawdClient {
  ClawdClient({
    this.url = 'ws://127.0.0.1:$kClawdPort',
    this.callTimeout = kDefaultCallTimeout,
  });

  final String url;

  /// How long to wait for a response before throwing [ClawdTimeoutError].
  final Duration callTimeout;

  WebSocketChannel? _channel;
  int _idCounter = 0;
  final Map<int, Completer<dynamic>> _pending = {};
  final StreamController<Map<String, dynamic>> _pushEvents =
      StreamController.broadcast();

  bool get isConnected => _channel != null;

  /// Stream of server-push events (session updates, git status, tool calls).
  Stream<Map<String, dynamic>> get pushEvents => _pushEvents.stream;

  Future<void> connect() async {
    _channel = WebSocketChannel.connect(Uri.parse(url));
    _channel!.stream.listen(
      _onMessage,
      onDone: _onDisconnect,
      onError: (_) => _onDisconnect(),
    );
  }

  void disconnect() {
    _channel?.sink.close();
    _channel = null;
  }

  /// Send a JSON-RPC 2.0 request and return the decoded result.
  ///
  /// Throws [ClawdDisconnectedError] if not connected or connection drops.
  /// Throws [ClawdRpcError] if the daemon returns an error response.
  /// Throws [ClawdTimeoutError] if no response arrives within [callTimeout].
  Future<T> call<T>(String method, [Map<String, dynamic>? params]) async {
    if (_channel == null) throw const ClawdDisconnectedError();

    final id = ++_idCounter;
    final completer = Completer<dynamic>();
    _pending[id] = completer;

    _channel!.sink.add(jsonEncode(
      RpcRequest(method: method, params: params, id: id).toJson(),
    ));

    try {
      return (await completer.future.timeout(
        callTimeout,
        onTimeout: () {
          _pending.remove(id);
          throw ClawdTimeoutError(method);
        },
      )) as T;
    } catch (_) {
      _pending.remove(id);
      rethrow;
    }
  }

  void _onMessage(dynamic raw) {
    final json = jsonDecode(raw as String) as Map<String, dynamic>;

    // Server-push notification — no id field
    if (!json.containsKey('id')) {
      _pushEvents.add(json);
      return;
    }

    final response = RpcResponse.fromJson(json);
    final completer = _pending.remove(response.id);
    if (completer == null) return;

    if (response.isError) {
      completer.completeError(ClawdRpcError(
        code: response.error!.code,
        message: response.error!.message,
      ));
    } else {
      completer.complete(response.result);
    }
  }

  void _onDisconnect() {
    _channel = null;
    for (final c in _pending.values) {
      c.completeError(const ClawdDisconnectedError());
    }
    _pending.clear();
  }
}
