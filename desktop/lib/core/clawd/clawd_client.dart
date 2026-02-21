import 'dart:async';
import 'dart:convert';
import 'package:web_socket_channel/web_socket_channel.dart';

// Default clawd daemon port
const int kClawdPort = 4300;

/// JSON-RPC 2.0 client for communicating with clawd over WebSocket.
///
/// Connects to ws://127.0.0.1:4300 (local) or wss://relay.clawde.io (remote).
class ClawdClient {
  ClawdClient({this.url = 'ws://127.0.0.1:$kClawdPort'});

  final String url;

  WebSocketChannel? _channel;
  int _idCounter = 0;
  final Map<int, Completer<dynamic>> _pending = {};
  final StreamController<Map<String, dynamic>> _notifications =
      StreamController.broadcast();

  bool get isConnected => _channel != null;

  /// Stream of server-push notifications (events, git status updates, etc.)
  Stream<Map<String, dynamic>> get notifications => _notifications.stream;

  Future<void> connect() async {
    _channel = WebSocketChannel.connect(Uri.parse(url));
    _channel!.stream.listen(
      _onMessage,
      onDone: _onDisconnect,
      onError: _onError,
    );
  }

  void disconnect() {
    _channel?.sink.close();
    _channel = null;
  }

  /// Send a JSON-RPC 2.0 request and return the result.
  Future<T> call<T>(String method, [Map<String, dynamic>? params]) async {
    final id = ++_idCounter;
    final completer = Completer<dynamic>();
    _pending[id] = completer;

    _channel!.sink.add(jsonEncode({
      'jsonrpc': '2.0',
      'id': id,
      'method': method,
      if (params != null) 'params': params,
    }));

    return (await completer.future) as T;
  }

  void _onMessage(dynamic raw) {
    final msg = jsonDecode(raw as String) as Map<String, dynamic>;

    if (msg.containsKey('id') && _pending.containsKey(msg['id'])) {
      // Response to a request
      final completer = _pending.remove(msg['id'])!;
      if (msg.containsKey('error')) {
        completer.completeError(ClawdError.fromJson(msg['error']));
      } else {
        completer.complete(msg['result']);
      }
    } else if (msg.containsKey('method')) {
      // Server-push notification
      _notifications.add(msg);
    }
  }

  void _onDisconnect() {
    _channel = null;
    for (final c in _pending.values) {
      c.completeError(const ClawdDisconnectedError());
    }
    _pending.clear();
  }

  void _onError(Object err) {
    _onDisconnect();
  }
}

class ClawdError implements Exception {
  const ClawdError({required this.code, required this.message});

  factory ClawdError.fromJson(Map<String, dynamic> json) {
    return ClawdError(
      code: json['code'] as int,
      message: json['message'] as String,
    );
  }

  final int code;
  final String message;

  @override
  String toString() => 'ClawdError($code): $message';
}

class ClawdDisconnectedError implements Exception {
  const ClawdDisconnectedError();

  @override
  String toString() => 'ClawdDisconnectedError: connection lost';
}
