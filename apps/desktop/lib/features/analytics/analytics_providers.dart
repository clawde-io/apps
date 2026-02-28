// SPDX-License-Identifier: MIT
import 'package:clawd_core/clawd_core.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

// ─── Models ──────────────────────────────────────────────────────────────────

class DailyCount {
  const DailyCount({required this.date, required this.count});
  final String date;
  final int count;
}

class PersonalAnalytics {
  const PersonalAnalytics({
    required this.linesWritten,
    required this.aiAssistPercent,
    required this.languages,
    required this.sessionsPerDay,
  });
  final int linesWritten;
  final double aiAssistPercent;
  final Map<String, int> languages;
  final List<DailyCount> sessionsPerDay;

  static PersonalAnalytics fromJson(Map<String, dynamic> json) {
    final rawDays = json['sessionsPerDay'] as List<dynamic>? ?? [];
    final days = rawDays.map((d) {
      final m = d as Map<String, dynamic>;
      return DailyCount(
        date: m['date'] as String? ?? '',
        count: (m['count'] as num?)?.toInt() ?? 0,
      );
    }).toList();

    final rawLangs = json['languages'] as Map<String, dynamic>? ?? {};
    final langs = rawLangs.map(
      (k, v) => MapEntry(k, (v as num?)?.toInt() ?? 0),
    );

    return PersonalAnalytics(
      linesWritten: (json['linesWritten'] as num?)?.toInt() ?? 0,
      aiAssistPercent: (json['aiAssistPercent'] as num?)?.toDouble() ?? 0.0,
      languages: langs,
      sessionsPerDay: days,
    );
  }
}

class ProviderBreakdown {
  const ProviderBreakdown({
    required this.provider,
    required this.sessions,
    required this.tokens,
    required this.costUsd,
    this.winRate,
  });
  final String provider;
  final int sessions;
  final int tokens;
  final double costUsd;
  final double? winRate;

  static ProviderBreakdown fromJson(Map<String, dynamic> json) {
    return ProviderBreakdown(
      provider: json['provider'] as String? ?? '',
      sessions: (json['sessions'] as num?)?.toInt() ?? 0,
      tokens: (json['tokens'] as num?)?.toInt() ?? 0,
      costUsd: (json['costUsd'] as num?)?.toDouble() ?? 0.0,
      winRate: (json['winRate'] as num?)?.toDouble(),
    );
  }
}

class AchievementData {
  const AchievementData({
    required this.id,
    required this.name,
    required this.description,
    required this.unlocked,
    this.unlockedAt,
  });
  final String id;
  final String name;
  final String description;
  final bool unlocked;
  final String? unlockedAt;

  static AchievementData fromJson(Map<String, dynamic> json) {
    return AchievementData(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      description: json['description'] as String? ?? '',
      unlocked: json['unlocked'] as bool? ?? false,
      unlockedAt: json['unlockedAt'] as String?,
    );
  }
}

// ─── Providers ───────────────────────────────────────────────────────────────

/// Fetch personal analytics from the daemon (`analytics.personal`).
final personalAnalyticsProvider = FutureProvider<PersonalAnalytics>((ref) async {
  final client = ref.read(daemonProvider.notifier).client;
  final result = await client.call('analytics.personal', {});
  final map = result as Map<String, dynamic>? ?? {};
  return PersonalAnalytics.fromJson(map);
});

/// Fetch per-provider breakdown from the daemon (`analytics.providerBreakdown`).
final providerBreakdownProvider = FutureProvider<List<ProviderBreakdown>>((ref) async {
  final client = ref.read(daemonProvider.notifier).client;
  final result = await client.call('analytics.providerBreakdown', {});
  final list = result as List<dynamic>? ?? [];
  return list
      .map((e) => ProviderBreakdown.fromJson(e as Map<String, dynamic>))
      .toList();
});

/// Fetch achievement list from the daemon (`achievements.list`).
final achievementsProvider = FutureProvider<List<AchievementData>>((ref) async {
  final client = ref.read(daemonProvider.notifier).client;
  final result = await client.call('achievements.list', {});
  final list = result as List<dynamic>? ?? [];
  return list
      .map((e) => AchievementData.fromJson(e as Map<String, dynamic>))
      .toList();
});
