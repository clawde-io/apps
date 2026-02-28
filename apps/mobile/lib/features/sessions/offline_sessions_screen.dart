// offline_sessions_screen.dart — Cached session browser for offline mode (Sprint RR MO.4).
//
// When the daemon is unreachable this screen loads sessions from the local
// OfflineCache SQLite DB. All actions that require a live connection are
// disabled (new session, send message). Browsing and reading are always available.

import 'package:clawde_mobile/data/offline_sync.dart';
import 'package:clawde_mobile/widgets/offline_banner.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

final _offlineSessionsProvider =
    FutureProvider.autoDispose<List<Map<String, dynamic>>>((ref) async {
  final cache = ref.watch(offlineCacheProvider);
  return cache.getSessions();
});

class OfflineSessionsScreen extends ConsumerWidget {
  const OfflineSessionsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final sessionsAsync = ref.watch(_offlineSessionsProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Sessions'),
        actions: [
          Padding(
            padding: const EdgeInsets.only(right: 12),
            child: Chip(
              label: const Text('Cached'),
              avatar: const Icon(Icons.archive_outlined, size: 14),
              backgroundColor: Colors.amber.shade100,
              labelStyle: TextStyle(
                fontSize: 11,
                color: Colors.amber.shade900,
              ),
            ),
          ),
        ],
      ),
      body: OfflineBanner(
        child: sessionsAsync.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (e, _) => Center(
            child: Text('Cache unavailable: $e',
                style: const TextStyle(color: Colors.red)),
          ),
          data: (sessions) {
            if (sessions.isEmpty) {
              return const Center(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(Icons.cloud_off, size: 48, color: Colors.grey),
                    SizedBox(height: 12),
                    Text('No cached sessions',
                        style: TextStyle(color: Colors.grey)),
                    SizedBox(height: 4),
                    Text('Connect to the daemon to load sessions',
                        style: TextStyle(fontSize: 12, color: Colors.grey)),
                  ],
                ),
              );
            }

            return ListView.builder(
              itemCount: sessions.length,
              itemBuilder: (context, index) {
                final session = sessions[index];
                final id = session['id'] as String;
                final repoPath = session['repo_path'] as String? ?? '';
                final status = session['status'] as String? ?? 'unknown';
                final msgCount = session['message_count'] as int? ?? 0;

                return ListTile(
                  leading: const CircleAvatar(
                    backgroundColor: Color(0xFF1E1E2E),
                    child: Icon(Icons.terminal, size: 18, color: Colors.white70),
                  ),
                  title: Text(
                    repoPath.split('/').last.isEmpty
                        ? repoPath
                        : repoPath.split('/').last,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  subtitle: Text(
                    '$status · $msgCount messages',
                    style: const TextStyle(fontSize: 12),
                  ),
                  trailing: const Icon(Icons.chevron_right, size: 18),
                  onTap: () => context.push('/offline/sessions/$id'),
                );
              },
            );
          },
        ),
      ),
    );
  }
}
