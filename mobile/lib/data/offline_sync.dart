// offline_sync.dart — Background sync: daemon → local offline cache (Sprint RR MO.2).
//
// Periodically replicates sessions and their recent messages from the daemon
// into the OfflineCache SQLite DB. When the daemon is unreachable, the cache
// provides read-only access to previous data.

import 'dart:async';

import 'package:clawd_client/clawd_client.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:clawde_mobile/data/offline_cache.dart';

/// Sync interval when daemon is reachable.
const _syncInterval = Duration(minutes: 2);

/// Number of most-recent messages to cache per session.
const _messageLimit = 200;

class OfflineSyncService {
  OfflineSyncService({required ClawdClient client, required OfflineCache cache})
      : _client = client,
        _cache = cache;

  final ClawdClient _client;
  final OfflineCache _cache;
  Timer? _timer;

  /// Start the periodic sync. Call once after daemon connects.
  void start() {
    _timer?.cancel();
    // Sync immediately, then on interval
    _sync();
    _timer = Timer.periodic(_syncInterval, (_) => _sync());
  }

  /// Stop syncing (e.g., when daemon disconnects).
  void stop() {
    _timer?.cancel();
    _timer = null;
  }

  Future<void> _sync() async {
    try {
      // Fetch session list
      final result = await _client.call('session.list', {});
      final sessions = (result['sessions'] as List<dynamic>? ?? [])
          .cast<Map<String, dynamic>>();

      for (final session in sessions) {
        await _cache.upsertSession(session);

        // Fetch last N messages for this session
        final sessionId = session['id'] as String;
        final msgResult = await _client.call('session.messages', {
          'session_id': sessionId,
          'limit': _messageLimit,
        });
        final messages = (msgResult['messages'] as List<dynamic>? ?? [])
            .cast<Map<String, dynamic>>();
        if (messages.isNotEmpty) {
          await _cache.upsertMessages(sessionId, messages);
        }
      }
    } catch (_) {
      // Sync failure is non-fatal — cache serves stale data
    }
  }

  void dispose() {
    stop();
  }
}

// ─── Riverpod providers ───────────────────────────────────────────────────────

final offlineCacheProvider = Provider<OfflineCache>((ref) {
  final cache = OfflineCache();
  ref.onDispose(cache.close);
  return cache;
});
