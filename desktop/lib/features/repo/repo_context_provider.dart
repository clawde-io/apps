import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';

/// Optionally pins the repo view to a specific path.
/// When null, the Files/Git screens follow the active session's repoPath.
final repoContextPathProvider = StateProvider<String?>((ref) => null);

/// The effective repo path shown in Files / Git screens.
/// Prefers [repoContextPathProvider] (pinned) over the active session's path.
final effectiveRepoPathProvider = Provider<String?>((ref) {
  final pinned = ref.watch(repoContextPathProvider);
  if (pinned != null) return pinned;
  return ref.watch(activeSessionProvider)?.repoPath;
});

/// Fetches live repo status for the effective repo path via the daemon.
/// Returns null when no path is available.
/// Auto-refreshes on `repo.statusChanged` push events (DR-07).
final activeRepoStatusProvider = FutureProvider<RepoStatus?>((ref) async {
  final path = ref.watch(effectiveRepoPathProvider);
  if (path == null) return null;

  // Subscribe to push events; invalidate self when repo changes.
  ref.listen(daemonPushEventsProvider, (_, next) {
    next.whenData((event) {
      if (event['method'] == 'repo.statusChanged') {
        ref.invalidateSelf();
      }
    });
  });

  final client = ref.read(daemonProvider.notifier).client;
  final result =
      await client.call<Map<String, dynamic>>('repo.status', {'path': path});
  return RepoStatus.fromJson(result);
});
