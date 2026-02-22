import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'daemon_provider.dart';

/// All sessions known to the daemon. Refreshed on connect and on
/// `session.created` / `session.updated` / `session.closed` push events.
class SessionListNotifier extends AsyncNotifier<List<Session>> {
  @override
  Future<List<Session>> build() async {
    // Re-fetch whenever the daemon reconnects.
    ref.listen(daemonProvider, (prev, next) {
      if (next.isConnected) refresh();
    });

    // Re-fetch on push events that change session state.
    ref.listen(daemonPushEventsProvider, (_, next) {
      next.whenData((event) {
        final method = event['method'] as String?;
        if (method != null &&
            (method == 'session.created' ||
                method == 'session.updated' ||
                method == 'session.closed')) {
          refresh();
        }
      });
    });

    return _fetch();
  }

  Future<List<Session>> _fetch() async {
    final client = ref.read(daemonProvider.notifier).client;
    final result = await client.call<List<dynamic>>('session.list');
    return result
        .map((j) => Session.fromJson(j as Map<String, dynamic>))
        .toList();
  }

  Future<void> refresh() async {
    state = const AsyncValue.loading();
    try {
      state = AsyncValue.data(await _fetch());
    } catch (e, st) {
      state = AsyncValue.error(e, st);
    }
  }

  Future<Session> create({
    required String repoPath,
    ProviderType provider = ProviderType.claude,
  }) async {
    final client = ref.read(daemonProvider.notifier).client;
    final result = await client.call<Map<String, dynamic>>(
      'session.create',
      {'repo_path': repoPath, 'provider': provider.name},
    );
    await refresh();
    return Session.fromJson(result);
  }

  Future<void> close(String sessionId) async {
    final client = ref.read(daemonProvider.notifier).client;
    await client.call<void>('session.close', {'session_id': sessionId});
    await refresh();
  }
}

final sessionListProvider =
    AsyncNotifierProvider<SessionListNotifier, List<Session>>(
  SessionListNotifier.new,
);

/// The currently focused session ID. Persisted in desktop/mobile navigation state.
final activeSessionIdProvider = StateProvider<String?>((ref) => null);

/// Derives the full Session object for the active session ID.
final activeSessionProvider = Provider<Session?>((ref) {
  final id = ref.watch(activeSessionIdProvider);
  if (id == null) return null;
  final sessions = ref.watch(sessionListProvider).valueOrNull ?? [];
  return sessions.where((s) => s.id == id).firstOrNull;
});
