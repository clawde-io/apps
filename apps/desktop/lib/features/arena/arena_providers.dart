// SPDX-License-Identifier: MIT
/// Riverpod providers for Arena Mode (Sprint K, AM.T01–AM.T06).
///
/// Wraps the `arena.*` RPC calls.  All providers degrade gracefully when the
/// daemon doesn't support the RPC yet — older builds won't crash.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';

// ─── State types ─────────────────────────────────────────────────────────────

/// Immutable state for the current arena session.
class ArenaSessionState {
  const ArenaSessionState({
    required this.arenaSessionId,
    required this.sessionAId,
    required this.sessionBId,
    required this.providerA,
    required this.providerB,
    this.votedWinner,
  });

  /// Arena session identifier returned by `arena.createSession`.
  final String arenaSessionId;

  /// Daemon session ID for provider A.
  final String sessionAId;

  /// Daemon session ID for provider B.
  final String sessionBId;

  /// Provider name for session A (revealed after vote).
  final String providerA;

  /// Provider name for session B (revealed after vote).
  final String providerB;

  /// Winning provider name after the user votes.  `null` = not yet voted.
  final String? votedWinner;

  /// Whether the user has already cast a vote for this arena session.
  bool get hasVoted => votedWinner != null;

  ArenaSessionState copyWith({String? votedWinner}) => ArenaSessionState(
        arenaSessionId: arenaSessionId,
        sessionAId: sessionAId,
        sessionBId: sessionBId,
        providerA: providerA,
        providerB: providerB,
        votedWinner: votedWinner ?? this.votedWinner,
      );
}

/// A single leaderboard entry aggregated from arena votes.
class LeaderboardEntry {
  const LeaderboardEntry({
    required this.provider,
    required this.taskType,
    required this.wins,
    required this.total,
    required this.winRate,
  });

  factory LeaderboardEntry.fromJson(Map<String, dynamic> json) =>
      LeaderboardEntry(
        provider: json['provider'] as String,
        taskType: json['task_type'] as String? ?? json['taskType'] as String? ?? 'general',
        wins: (json['wins'] as num).toInt(),
        total: (json['total'] as num).toInt(),
        winRate: (json['win_rate'] as num?)?.toDouble() ??
            (json['winRate'] as num?)?.toDouble() ??
            0.0,
      );

  final String provider;
  final String taskType;
  final int wins;
  final int total;
  final double winRate;
}

// ─── Providers ────────────────────────────────────────────────────────────────

/// Active arena session state.  `null` when no arena session is in progress.
final arenaSessionProvider =
    StateProvider<ArenaSessionState?>((ref) => null);

/// Create a new arena session and update [arenaSessionProvider].
///
/// Usage:
/// ```dart
/// await ref.read(arenaCreateProvider.notifier).create(
///   repoPath: '/path/to/repo',
///   providerA: 'claude',
///   providerB: 'codex',
///   prompt: 'Refactor this function',
/// );
/// ```
class ArenaCreateNotifier extends AsyncNotifier<void> {
  @override
  Future<void> build() async {}

  Future<void> create({
    required String repoPath,
    required String providerA,
    required String providerB,
    required String prompt,
  }) async {
    state = const AsyncValue.loading();
    try {
      final client = ref.read(daemonProvider.notifier).client;
      final result = await client.call<Map<String, dynamic>>(
        'arena.createSession',
        {
          'repoPath': repoPath,
          'providerA': providerA,
          'providerB': providerB,
          'prompt': prompt,
        },
      );

      final arenaState = ArenaSessionState(
        arenaSessionId: result['arenaSessionId'] as String,
        sessionAId: result['sessionAId'] as String,
        sessionBId: result['sessionBId'] as String,
        providerA: result['providerA'] as String,
        providerB: result['providerB'] as String,
      );
      ref.read(arenaSessionProvider.notifier).state = arenaState;
      state = const AsyncValue.data(null);
    } catch (e, st) {
      state = AsyncValue.error(e, st);
    }
  }
}

final arenaCreateProvider = AsyncNotifierProvider<ArenaCreateNotifier, void>(
  ArenaCreateNotifier.new,
);

/// Record a vote for the current arena session.
class ArenaVoteNotifier extends AsyncNotifier<void> {
  @override
  Future<void> build() async {}

  Future<void> vote({
    required String arenaSessionId,
    required String winnerProvider,
    String taskType = 'general',
  }) async {
    state = const AsyncValue.loading();
    try {
      final client = ref.read(daemonProvider.notifier).client;
      await client.call<Map<String, dynamic>>(
        'arena.vote',
        {
          'arenaSessionId': arenaSessionId,
          'winnerProvider': winnerProvider,
          'taskType': taskType,
        },
      );

      // Update local state to reveal providers.
      final current = ref.read(arenaSessionProvider);
      if (current != null) {
        ref.read(arenaSessionProvider.notifier).state =
            current.copyWith(votedWinner: winnerProvider);
      }

      // Refresh leaderboard.
      ref.invalidate(arenaLeaderboardProvider);

      state = const AsyncValue.data(null);
    } catch (e, st) {
      state = AsyncValue.error(e, st);
    }
  }
}

final arenaVoteProvider = AsyncNotifierProvider<ArenaVoteNotifier, void>(
  ArenaVoteNotifier.new,
);

/// Leaderboard entries fetched from the daemon.
///
/// Pass an optional task type string to filter entries (e.g. "debug").
/// Pass `null` for the full leaderboard with aggregate rows.
final arenaLeaderboardProvider =
    FutureProvider.autoDispose.family<List<LeaderboardEntry>, String?>(
  (ref, taskType) async {
    final client = ref.read(daemonProvider.notifier).client;
    final params = taskType != null ? {'taskType': taskType} : <String, dynamic>{};
    final result = await client.call<Map<String, dynamic>>(
      'arena.leaderboard',
      params,
    );
    final entries = result['entries'] as List<dynamic>? ?? [];
    return entries
        .whereType<Map<String, dynamic>>()
        .map(LeaderboardEntry.fromJson)
        .toList();
  },
);
