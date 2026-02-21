import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'daemon_provider.dart';

/// Messages for a specific session. Keyed by session ID.
/// Appends new messages on `message.appended` push events.
class MessageListNotifier
    extends FamilyAsyncNotifier<List<Message>, String> {
  @override
  Future<List<Message>> build(String sessionId) async {
    ref.listen(daemonPushEventsProvider, (_, next) {
      next.whenData((event) {
        final method = event['method'] as String?;
        if (method == 'message.appended') {
          final params = event['params'] as Map<String, dynamic>?;
          if (params?['session_id'] == sessionId) {
            _appendFromEvent(params!);
          }
        }
      });
    });

    return _fetch();
  }

  Future<List<Message>> _fetch() async {
    final client = ref.read(daemonProvider.notifier).client;
    final result = await client.call<List<dynamic>>(
      'message.list',
      {'session_id': arg},
    );
    return result
        .map((j) => Message.fromJson(j as Map<String, dynamic>))
        .toList();
  }

  void _appendFromEvent(Map<String, dynamic> params) {
    final newMessage = Message.fromJson(params);
    state.whenData((messages) {
      state = AsyncValue.data([...messages, newMessage]);
    });
  }

  Future<void> refresh() async {
    state = const AsyncValue.loading();
    try {
      state = AsyncValue.data(await _fetch());
    } catch (e, st) {
      state = AsyncValue.error(e, st);
    }
  }

  Future<void> send(String content) async {
    final client = ref.read(daemonProvider.notifier).client;
    await client.call<void>('session.sendMessage', {
      'session_id': arg,
      'content': content,
    });
    // The response message will arrive via push event and be appended above.
  }
}

final messageListProvider = AsyncNotifierProviderFamily<MessageListNotifier,
    List<Message>, String>(
  MessageListNotifier.new,
);
