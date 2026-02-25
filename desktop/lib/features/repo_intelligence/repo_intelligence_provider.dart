/// Riverpod providers for the Repo Intelligence subsystem (Sprint F, RI.T19).
///
/// Wraps the `repo.scan`, `repo.profile`, `repo.driftScore`, and
/// `repo.generateArtifacts` RPCs.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';

// ─── Profile provider ─────────────────────────────────────────────────────────

/// Watches the stored repo profile for [repoPath].
///
/// Returns null if the repo has not been scanned yet.
/// Auto-triggers a background scan if no profile is found on first load.
final repoProfileProvider =
    FutureProvider.family<Map<String, dynamic>?, String>(
  (ref, repoPath) async {
    final client = ref.read(daemonProvider.notifier).client;
    final result = await client.call<Map<String, dynamic>>(
        'repo.profile', {'repoPath': repoPath});
    final profile = result['profile'];
    if (profile == null) {
      // Trigger a background scan; panel will show "Scan repo" until done
      client
          .call<Map<String, dynamic>>('repo.scan', {'repoPath': repoPath})
          .ignore();
      return null;
    }
    return Map<String, dynamic>.from(profile as Map);
  },
);

// ─── Drift score provider ─────────────────────────────────────────────────────

/// Returns the 0–100 drift score for [repoPath].
final repoDriftScoreProvider = FutureProvider.family<int, String>(
  (ref, repoPath) async {
    final client = ref.read(daemonProvider.notifier).client;
    final result = await client.call<Map<String, dynamic>>(
        'repo.driftScore', {'repoPath': repoPath});
    return (result['score'] as num?)?.toInt() ?? 0;
  },
);

// ─── Actions ─────────────────────────────────────────────────────────────────

/// Actions provider (non-reactive — use for fire-and-forget calls).
final repoScanActionsProvider = Provider<RepoScanActions>(
  (ref) => RepoScanActions(ref),
);

class RepoScanActions {
  const RepoScanActions(this._ref);
  final Ref _ref;

  /// Trigger a full repo scan (refreshes the profile provider on completion).
  Future<void> scan(String repoPath) async {
    final client = _ref.read(daemonProvider.notifier).client;
    await client
        .call<Map<String, dynamic>>('repo.scan', {'repoPath': repoPath});
    // Invalidate the profile cache so the panel re-renders
    _ref.invalidate(repoProfileProvider(repoPath));
    _ref.invalidate(repoDriftScoreProvider(repoPath));
  }

  /// Call `repo.generateArtifacts` and return the result list.
  Future<List<Map<String, dynamic>>> generateArtifacts(
    String repoPath, {
    bool overwrite = false,
  }) async {
    final client = _ref.read(daemonProvider.notifier).client;
    final result = await client.call<Map<String, dynamic>>(
      'repo.generateArtifacts',
      {'repoPath': repoPath, 'overwrite': overwrite},
    );
    final artifacts = result['artifacts'] as List? ?? [];
    return artifacts
        .map((a) => Map<String, dynamic>.from(a as Map))
        .toList();
  }
}
