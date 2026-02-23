import 'dart:async';
import 'dart:convert';
import 'dart:developer' as dev;

import 'package:clawd_proto/clawd_proto.dart';
import 'package:flutter/foundation.dart' show visibleForTesting;
import 'package:web_socket_channel/web_socket_channel.dart';

import 'exceptions.dart';
import 'relay_crypto.dart';

/// Default clawd daemon port.
const int kClawdPort = 4300;

/// Default timeout for RPC calls. Long-running operations (e.g. session.create,
/// session.sendMessage) inherit this; callers can override per-call if needed.
const Duration kDefaultCallTimeout = Duration(seconds: 30);

// ─── Relay connection options ─────────────────────────────────────────────────

/// Options for connecting to the daemon via the ClawDE relay server.
///
/// When supplied to [ClawdClient], the client performs the relay handshake
/// (type: "connect") before authenticating with the daemon.  If [enableE2e]
/// is true (the default) the client also performs the X25519 key exchange and
/// encrypts all subsequent frames with ChaCha20-Poly1305.
class RelayOptions {
  const RelayOptions({
    required this.daemonId,
    required this.userToken,
    this.enableE2e = true,
  });

  /// The daemon's unique ID (registered with the relay server).
  final String daemonId;

  /// JWT issued by nhost — used to authenticate with the relay server.
  final String userToken;

  /// Whether to perform E2E key exchange (recommended; default true).
  final bool enableE2e;
}

// ─── Client ───────────────────────────────────────────────────────────────────

/// JSON-RPC 2.0 WebSocket client for the clawd daemon.
///
/// Can connect directly (LAN) or via the ClawDE relay server (remote).
/// When [relayOptions] is supplied the client performs the relay handshake
/// and optional E2E encryption automatically.
///
/// Usage (local):
/// ```dart
/// final client = ClawdClient(authToken: token);
/// await client.connect();
/// final session = Session.fromJson(await client.call('session.create', {...}));
/// ```
///
/// Usage (relay with E2E):
/// ```dart
/// final client = ClawdClient(
///   url: 'wss://api.clawde.io/relay/ws',
///   relayOptions: RelayOptions(daemonId: id, userToken: jwt),
///   authToken: daemonToken,
/// );
/// await client.connect();
/// ```
class ClawdClient {
  ClawdClient({
    this.url = 'ws://127.0.0.1:$kClawdPort',
    this.callTimeout = kDefaultCallTimeout,
    this.authToken,
    this.relayOptions,
    @visibleForTesting WebSocketChannel Function(Uri)? channelFactory,
  }) : _channelFactory = channelFactory ?? WebSocketChannel.connect;

  final String url;
  final Duration callTimeout;

  /// Auth token for the daemon.  When set, [connect] sends a `daemon.auth`
  /// RPC immediately after the WebSocket (and optional relay/E2E) handshake.
  final String? authToken;

  /// When set, the client connects via the ClawDE relay server instead of
  /// connecting directly to the daemon.
  final RelayOptions? relayOptions;

  final WebSocketChannel Function(Uri) _channelFactory;

  WebSocketChannel? _channel;
  StreamSubscription<dynamic>? _subscription;
  int _idCounter = 0;
  final Map<int, Completer<dynamic>> _pending = {};
  final StreamController<Map<String, dynamic>> _pushEvents =
      StreamController.broadcast();

  // E2E session (set after handshake, null for direct/unencrypted connections).
  RelayE2eSession? _e2eSession;

  bool get isConnected => _channel != null;

  /// Stream of server-push events (session updates, git status, tool calls).
  Stream<Map<String, dynamic>> get pushEvents => _pushEvents.stream;

  Future<void> connect() async {
    _channel = _channelFactory(Uri.parse(url));
    _subscription = _channel!.stream.listen(
      (raw) => _processMessage(raw),
      onDone: _onDisconnect,
      onError: (_) => _onDisconnect(),
    );

    final relay = relayOptions;
    if (relay != null) {
      // 1. Send relay connect message.
      _sendRaw(jsonEncode({
        'type': 'connect',
        'daemonId': relay.daemonId,
        'token': relay.userToken,
      }));

      // 2. Wait for relay to confirm the connection.
      await _waitForPushEvent(
        'connected',
        timeout: const Duration(seconds: 10),
      );

      // 3. E2E handshake.
      if (relay.enableE2e) {
        await _performE2eHandshake();
      }
    }

    // 4. Authenticate with the daemon (encrypted when E2E is active).
    final token = authToken;
    if (token != null && token.isNotEmpty) {
      await call<Map<String, dynamic>>('daemon.auth', {'token': token});
    }
  }

  void disconnect() {
    _subscription?.cancel();
    _subscription = null;
    _channel?.sink.close();
    _channel = null;
    _e2eSession = null;
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

    final text = jsonEncode(
      RpcRequest(method: method, params: params, id: id).toJson(),
    );
    await _sendFrame(text);

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

  // ─── Internal ─────────────────────────────────────────────────────────────

  void _sendRaw(String text) {
    _channel?.sink.add(text);
  }

  /// Send a frame, encrypting it if E2E is active.
  Future<void> _sendFrame(String text) async {
    final session = _e2eSession;
    if (session != null) {
      final payload = await session.encrypt(text);
      _sendRaw(jsonEncode({'type': 'e2e', 'payload': payload}));
    } else {
      _sendRaw(text);
    }
  }

  void _processMessage(dynamic raw) {
    _handleMessageAsync(raw).catchError((Object e) {
      dev.log('message processing error: $e', name: 'clawd_client');
    });
  }

  Future<void> _handleMessageAsync(dynamic raw) async {
    Map<String, dynamic> json;
    try {
      json = jsonDecode(raw as String) as Map<String, dynamic>;
    } catch (_) {
      return;
    }

    // Decrypt E2E frames.
    if (json['type'] == 'e2e') {
      final session = _e2eSession;
      if (session == null) return;
      final payload = json['payload'] as String?;
      if (payload == null) return;
      try {
        final decrypted = await session.decrypt(payload);
        json = jsonDecode(decrypted) as Map<String, dynamic>;
      } catch (e) {
        dev.log('E2E decrypt failed: $e', name: 'clawd_client');
        return;
      }
    }

    // Relay/protocol messages (no `id`) → push events stream.
    if (!json.containsKey('id') || json['id'] == null) {
      _pushEvents.add(json);
      return;
    }

    // JSON-RPC response → complete pending call.
    final response = RpcResponse.fromJson(json);
    final completer = _pending.remove(response.id);
    if (completer == null) return;

    if (response.isError) {
      final err = response.error!;
      dev.log(
        'RPC error [${err.code}]: ${err.message}',
        name: 'clawd_client',
      );
      completer.completeError(ClawdRpcError(
        code: err.code,
        message: err.message,
      ));
    } else {
      completer.complete(response.result);
    }
  }

  // ─── Task API ──────────────────────────────────────────────────────────────

  /// List tasks with optional filters.
  Future<List<AgentTask>> listTasks({
    String? repoPath,
    String? status,
    String? agent,
    String? severity,
    String? phase,
    int? limit,
    int? offset,
  }) async {
    final params = <String, dynamic>{
      if (repoPath != null) 'repo_path': repoPath,
      if (status != null) 'status': status,
      if (agent != null) 'agent': agent,
      if (severity != null) 'severity': severity,
      if (phase != null) 'phase': phase,
      if (limit != null) 'limit': limit,
      if (offset != null) 'offset': offset,
    };
    final result = await call<Map<String, dynamic>>('tasks.list', params);
    final list = result['tasks'] as List<dynamic>? ?? [];
    return list.map((e) => AgentTask.fromJson(e as Map<String, dynamic>)).toList();
  }

  /// Get a single task by ID.
  Future<AgentTask> getTask(String taskId) async {
    final result = await call<Map<String, dynamic>>('tasks.get', {'task_id': taskId});
    return AgentTask.fromJson(result['task'] as Map<String, dynamic>);
  }

  /// Atomically claim a task for an agent. Returns the claimed task.
  Future<({AgentTask task, bool isResume})> claimTask(
    String taskId,
    String agentId,
  ) async {
    final result = await call<Map<String, dynamic>>('tasks.claim', {
      'task_id': taskId,
      'agent_id': agentId,
    });
    return (
      task: AgentTask.fromJson(result['task'] as Map<String, dynamic>),
      isResume: result['is_resume'] as bool? ?? false,
    );
  }

  /// Release a claimed task back to pending.
  Future<void> releaseTask(String taskId, String agentId) async {
    await call<Map<String, dynamic>>('tasks.release', {
      'task_id': taskId,
      'agent_id': agentId,
    });
  }

  /// Send a heartbeat for an in-progress task.
  Future<void> taskHeartbeat(String taskId, String agentId) async {
    await call<Map<String, dynamic>>('tasks.heartbeat', {
      'task_id': taskId,
      'agent_id': agentId,
    });
  }

  /// Update task status.
  Future<AgentTask> updateTaskStatus(
    String taskId,
    String status, {
    String? notes,
    String? blockReason,
    String? agentId,
  }) async {
    final result = await call<Map<String, dynamic>>('tasks.updateStatus', {
      'task_id': taskId,
      'status': status,
      if (notes != null) 'notes': notes,
      if (blockReason != null) 'block_reason': blockReason,
      if (agentId != null) 'agent_id': agentId,
    });
    return AgentTask.fromJson(result['task'] as Map<String, dynamic>);
  }

  /// Add a new task to the queue.
  Future<AgentTask> addTask(Map<String, dynamic> params) async {
    final result = await call<Map<String, dynamic>>('tasks.addTask', params);
    return AgentTask.fromJson(result['task'] as Map<String, dynamic>);
  }

  /// Log an activity entry (auto type).
  Future<String> logActivity({
    required String agentId,
    required String action,
    required String repoPath,
    String? taskId,
    String? phase,
    String? detail,
    String? entryType,
  }) async {
    final result = await call<Map<String, dynamic>>('tasks.logActivity', {
      'agent_id': agentId,
      'action': action,
      'repo_path': repoPath,
      if (taskId != null) 'task_id': taskId,
      if (phase != null) 'phase': phase,
      if (detail != null) 'detail': detail,
      if (entryType != null) 'entry_type': entryType,
    });
    return result['id'] as String;
  }

  /// Post a human-readable note attached to a task.
  Future<String> postNote({
    required String agentId,
    required String note,
    required String repoPath,
    String? taskId,
    String? phase,
  }) async {
    final result = await call<Map<String, dynamic>>('tasks.note', {
      'agent_id': agentId,
      'note': note,
      'repo_path': repoPath,
      if (taskId != null) 'task_id': taskId,
      if (phase != null) 'phase': phase,
    });
    return result['id'] as String;
  }

  /// Query activity log entries.
  Future<List<ActivityLogEntry>> queryActivity({
    String? repoPath,
    String? taskId,
    String? agent,
    String? phase,
    String? entryType,
    String? action,
    int? since,
    int? limit,
    int? offset,
  }) async {
    final result = await call<Map<String, dynamic>>('tasks.activity', {
      if (repoPath != null) 'repo_path': repoPath,
      if (taskId != null) 'task_id': taskId,
      if (agent != null) 'agent': agent,
      if (phase != null) 'phase': phase,
      if (entryType != null) 'entry_type': entryType,
      if (action != null) 'action': action,
      if (since != null) 'since': since,
      if (limit != null) 'limit': limit,
      if (offset != null) 'offset': offset,
    });
    final list = result['entries'] as List<dynamic>? ?? [];
    return list
        .map((e) => ActivityLogEntry.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  /// Get a summary count of tasks by status.
  Future<TaskSummary> taskSummary({String? repoPath}) async {
    final result = await call<Map<String, dynamic>>(
      'tasks.summary',
      {if (repoPath != null) 'repo_path': repoPath},
    );
    return TaskSummary.fromJson(result);
  }

  /// Import tasks from a planning markdown document.
  Future<int> tasksFromPlanning(String path, String repoPath) async {
    final result = await call<Map<String, dynamic>>('tasks.fromPlanning', {
      'path': path,
      'repo_path': repoPath,
    });
    return (result['imported'] as num?)?.toInt() ?? 0;
  }

  /// Sync active.md → DB.
  Future<int> syncTasks(String repoPath) async {
    final result = await call<Map<String, dynamic>>('tasks.sync', {
      'repo_path': repoPath,
    });
    return (result['synced'] as num?)?.toInt() ?? 0;
  }

  // ─── Agent registry API ────────────────────────────────────────────────────

  /// Register this agent with the daemon.
  Future<AgentView> registerAgent({
    required String agentId,
    String agentType = 'claude',
    String? sessionId,
    String repoPath = '',
  }) async {
    final result = await call<Map<String, dynamic>>('tasks.agents.register', {
      'agent_id': agentId,
      'agent_type': agentType,
      if (sessionId != null) 'session_id': sessionId,
      'repo_path': repoPath,
    });
    return AgentView.fromJson(result['agent'] as Map<String, dynamic>);
  }

  /// List registered agents.
  Future<List<AgentView>> listAgents({String? repoPath}) async {
    final result = await call<Map<String, dynamic>>(
      'tasks.agents.list',
      {if (repoPath != null) 'repo_path': repoPath},
    );
    final list = result['agents'] as List<dynamic>? ?? [];
    return list
        .map((e) => AgentView.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  /// Send an agent heartbeat.
  Future<void> agentHeartbeat(String agentId) async {
    await call<Map<String, dynamic>>('tasks.agents.heartbeat', {
      'agent_id': agentId,
    });
  }

  /// Mark an agent as disconnected.
  Future<void> agentDisconnect(String agentId) async {
    await call<Map<String, dynamic>>('tasks.agents.disconnect', {
      'agent_id': agentId,
    });
  }

  // ─── AFS API ───────────────────────────────────────────────────────────────

  /// Scaffold a .claude/ directory tree at the given path.
  Future<List<String>> afsInit(String path) async {
    final result = await call<Map<String, dynamic>>('afs.init', {'path': path});
    final created = result['created'] as List<dynamic>? ?? [];
    return created.cast<String>();
  }

  /// Get AFS status for a repo.
  Future<Map<String, dynamic>> afsStatus(String repoPath) async {
    return call<Map<String, dynamic>>('afs.status', {'repo_path': repoPath});
  }

  /// Register a project path for AFS file watching.
  Future<void> afsRegister(String repoPath) async {
    await call<Map<String, dynamic>>('afs.register', {'repo_path': repoPath});
  }

  // ─── Internal ─────────────────────────────────────────────────────────────

  void _onDisconnect() {
    dev.log('WebSocket disconnected ($url)', name: 'clawd_client');
    _channel = null;
    _e2eSession = null;
    for (final c in _pending.values) {
      c.completeError(const ClawdDisconnectedError());
    }
    _pending.clear();
  }

  // ─── Relay/E2E helpers ─────────────────────────────────────────────────────

  Future<void> _performE2eHandshake() async {
    final handshake = await RelayE2eHandshake.create();

    // Send client hello unencrypted — server needs our pubkey to derive the key.
    _sendRaw(jsonEncode({
      'type': 'e2e_hello',
      'pubkey': handshake.clientPubkeyB64,
    }));

    // Wait for server hello.
    final serverHello = await _waitForPushEvent(
      'e2e_hello',
      timeout: const Duration(seconds: 10),
    );
    final serverPubkey = serverHello['pubkey'] as String?;
    if (serverPubkey == null) {
      throw Exception('relay e2e_hello missing pubkey');
    }

    _e2eSession = await handshake.complete(serverPubkey);
    dev.log('E2E encryption established', name: 'clawd_client');
  }

  Future<Map<String, dynamic>> _waitForPushEvent(
    String type, {
    required Duration timeout,
  }) async {
    final completer = Completer<Map<String, dynamic>>();
    late StreamSubscription<Map<String, dynamic>> sub;
    sub = _pushEvents.stream.listen((event) {
      if (event['type'] == type && !completer.isCompleted) {
        sub.cancel();
        completer.complete(event);
      }
    });
    try {
      return await completer.future.timeout(timeout);
    } catch (_) {
      sub.cancel();
      rethrow;
    }
  }
}
