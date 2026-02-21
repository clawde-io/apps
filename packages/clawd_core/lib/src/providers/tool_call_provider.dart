import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'daemon_provider.dart';

/// Pending tool calls for a session, waiting for user approval or auto-approved.
/// Updated via `tool_call.pending` and `tool_call.completed` push events.
class ToolCallNotifier
    extends FamilyAsyncNotifier<List<ToolCall>, String> {
  @override
  Future<List<ToolCall>> build(String sessionId) async {
    ref.listen(daemonPushEventsProvider, (_, next) {
      next.whenData((event) {
        final method = event['method'] as String?;
        if (method == 'tool_call.pending' || method == 'tool_call.completed') {
          final params = event['params'] as Map<String, dynamic>?;
          if (params?['session_id'] == sessionId) {
            refresh();
          }
        }
      });
    });

    return _fetch();
  }

  Future<List<ToolCall>> _fetch() async {
    final client = ref.read(daemonProvider.notifier).client;
    final result = await client.call<List<dynamic>>(
      'tool_call.list',
      {'session_id': arg, 'status': 'pending'},
    );
    return result
        .map((j) => ToolCall.fromJson(j as Map<String, dynamic>))
        .toList();
  }

  Future<void> refresh() async {
    try {
      state = AsyncValue.data(await _fetch());
    } catch (e, st) {
      state = AsyncValue.error(e, st);
    }
  }

  Future<void> approve(String toolCallId) async {
    final client = ref.read(daemonProvider.notifier).client;
    await client.call<void>('tool_call.approve', {'tool_call_id': toolCallId});
    await refresh();
  }

  Future<void> reject(String toolCallId) async {
    final client = ref.read(daemonProvider.notifier).client;
    await client.call<void>('tool_call.reject', {'tool_call_id': toolCallId});
    await refresh();
  }
}

final toolCallProvider = AsyncNotifierProviderFamily<ToolCallNotifier,
    List<ToolCall>, String>(
  ToolCallNotifier.new,
);

/// Count of pending tool calls across all sessions — drives badge indicators.
final pendingToolCallCountProvider = Provider<int>((ref) {
  final activeSessionId = ref.watch(
    Provider((ref) => null as String?), // overridden by apps
  );
  if (activeSessionId == null) return 0;
  return ref
          .watch(toolCallProvider(activeSessionId))
          .valueOrNull
          ?.length ??
      0;
});
