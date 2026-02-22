import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'daemon_provider.dart';

/// Messages for a specific session. Keyed by session ID.
/// Appends/updates messages on `session.messageCreated` and
/// `session.messageUpdated` push events.
/// Supports ME-03 pagination via [loadMore].
class MessageListNotifier
    extends FamilyAsyncNotifier<List<Message>, String> {
  /// ID of the oldest loaded message — used as `before` cursor for loadMore.
  String? _oldestMessageId;

  @override
  Future<List<Message>> build(String sessionId) async {
    ref.listen(daemonPushEventsProvider, (_, next) {
      next.whenData((event) {
        final method = event['method'] as String?;
        if (method == null) return;
        final params = event['params'] as Map<String, dynamic>?;
        if (params == null) return;

        if (method == 'session.messageCreated') {
          final msgJson = params['message'] as Map<String, dynamic>?;
          if (msgJson != null) _appendMessage(Message.fromJson(msgJson));
        } else if (method == 'session.messageUpdated') {
          final msgId = params['messageId'] as String?;
          final content = params['content'] as String?;
          final status = params['status'] as String?;
          if (msgId != null) _updateMessage(msgId, content, status);
        }
      });
    });

    final messages = await _fetchPage();
    if (messages.isNotEmpty) {
      _oldestMessageId = messages.first.id;
    }
    return messages;
  }

  /// Fetches one page (20 messages) optionally before [before] cursor.
  Future<List<Message>> _fetchPage({String? before}) async {
    final client = ref.read(daemonProvider.notifier).client;
    final params = <String, dynamic>{
      'sessionId': arg,
      'limit': 20,
      if (before != null) 'before': before,
    };
    final result = await client.call<List<dynamic>>(
      'session.getMessages',
      params,
    );
    return result
        .map((j) => Message.fromJson(j as Map<String, dynamic>))
        .toList();
  }

  void _appendMessage(Message msg) {
    state.whenData((messages) {
      state = AsyncValue.data([...messages, msg]);
    });
  }

  void _updateMessage(String msgId, String? content, String? status) {
    state.whenData((messages) {
      state = AsyncValue.data(
        messages.map((m) {
          if (m.id != msgId) return m;
          return Message(
            id: m.id,
            sessionId: m.sessionId,
            role: m.role,
            content: content ?? m.content,
            status: status ?? m.status,
            createdAt: m.createdAt,
            metadata: m.metadata,
          );
        }).toList(),
      );
    });
  }

  Future<void> refresh() async {
    state = const AsyncValue.loading();
    try {
      final messages = await _fetchPage();
      if (messages.isNotEmpty) _oldestMessageId = messages.first.id;
      state = AsyncValue.data(messages);
    } catch (e, st) {
      state = AsyncValue.error(e, st);
    }
  }

  /// ME-03: Load older messages and prepend them to the current list.
  Future<void> loadMore() async {
    final current = state.valueOrNull;
    if (current == null || _oldestMessageId == null) return;
    final older = await _fetchPage(before: _oldestMessageId);
    if (older.isEmpty) return;
    _oldestMessageId = older.first.id;
    state = AsyncValue.data([...older, ...current]);
  }

  /// SH-03: Pending queue — messages typed while offline are sent on reconnect.
  final List<String> _pendingQueue = [];

  Future<void> send(String content) async {
    final daemonState = ref.read(daemonProvider);
    if (!daemonState.isConnected) {
      // Queue the message and add a pending-state placeholder to the UI.
      _pendingQueue.add(content);
      _appendMessage(Message(
        id: 'pending-${DateTime.now().millisecondsSinceEpoch}',
        sessionId: arg,
        role: MessageRole.user,
        content: content,
        status: 'pending',
        createdAt: DateTime.now(),
        metadata: const {},
      ));
      // When daemon reconnects, drain the queue.
      ref.listen(daemonProvider, (prev, next) {
        if (!next.isConnected) return;
        if (prev?.isConnected == true) return; // already connected — skip
        _drainQueue();
      });
      return;
    }
    final client = ref.read(daemonProvider.notifier).client;
    await client.call<void>('session.sendMessage', {
      'sessionId': arg,
      'content': content,
    });
    // The response message will arrive via push event and be appended above.
  }

  Future<void> _drainQueue() async {
    while (_pendingQueue.isNotEmpty) {
      final content = _pendingQueue.removeAt(0);
      try {
        final client = ref.read(daemonProvider.notifier).client;
        await client.call<void>('session.sendMessage', {
          'sessionId': arg,
          'content': content,
        });
      } catch (_) {
        // Put it back at the front and stop draining — will retry next connect.
        _pendingQueue.insert(0, content);
        break;
      }
    }
  }
}

final messageListProvider = AsyncNotifierProviderFamily<MessageListNotifier,
    List<Message>, String>(
  MessageListNotifier.new,
);
