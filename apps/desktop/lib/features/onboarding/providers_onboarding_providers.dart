/// Riverpod providers for the Provider Onboarding subsystem (Sprint I, PO.T06–PO.T19).
///
/// Wraps the gci.generate, provider.checkAll, and account.capabilities RPCs.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';

// ─── Provider status ──────────────────────────────────────────────────────────

/// Loads the status of all three providers from the daemon.
///
/// Returns a map of provider name → status map.
/// Invalidated when the user triggers a re-check.
final providerStatusProvider = FutureProvider<Map<String, ProviderStatus>>(
  (ref) async {
    final client = ref.read(daemonProvider.notifier).client;
    final result =
        await client.call<Map<String, dynamic>>('provider.checkAll', {});

    final rawProviders = result['providers'] as Map<String, dynamic>? ?? {};
    return rawProviders.map(
      (key, value) => MapEntry(
        key,
        ProviderStatus.fromJson(Map<String, dynamic>.from(value as Map)),
      ),
    );
  },
);

// ─── Questionnaire state ──────────────────────────────────────────────────────

/// Mutable notifier that holds the in-progress questionnaire answers.
///
/// Answers are built up across the 7 question fields in QuestionnaireStep.
/// Call [QuestionnaireNotifier.submit] when the user completes the form.
class QuestionnaireNotifier extends Notifier<QuestionnaireState> {
  @override
  QuestionnaireState build() => const QuestionnaireState();

  void setPrimaryLanguages(List<String> langs) {
    state = state.copyWith(primaryLanguages: langs);
  }

  void setProjectTypes(List<String> types) {
    state = state.copyWith(projectTypes: types);
  }

  void setTeamSize(String size) {
    state = state.copyWith(teamSize: size);
  }

  void setAutonomyLevel(String level) {
    state = state.copyWith(autonomyLevel: level);
  }

  void setStyleRules(String style) {
    state = state.copyWith(styleRules: style);
  }

  void setGitWorkflow(String workflow) {
    state = state.copyWith(gitWorkflow: workflow);
  }

  void togglePainPoint(String point) {
    final current = List<String>.from(state.painPoints);
    if (current.contains(point)) {
      current.remove(point);
    } else {
      current.add(point);
    }
    state = state.copyWith(painPoints: current);
  }

  /// Resets the questionnaire to its initial state.
  void reset() {
    state = const QuestionnaireState();
  }
}

final questionnaireStateProvider =
    NotifierProvider<QuestionnaireNotifier, QuestionnaireState>(
  QuestionnaireNotifier.new,
);

// ─── GCI preview ─────────────────────────────────────────────────────────────

/// Generates the GCI preview content by calling gci.generate on the daemon.
///
/// This is a family provider keyed on the questionnaire answers JSON string
/// so that it re-runs whenever the answers change and the user taps "Preview".
final gciPreviewProvider =
    FutureProvider.family<GciPreviewResult, String>((ref, answersJson) async {
  final client = ref.read(daemonProvider.notifier).client;

  final answers = answersJsonToMap(answersJson);
  final result = await client.call<Map<String, dynamic>>(
    'gci.generate',
    {'answers': answers},
  );

  return GciPreviewResult(
    path: result['path'] as String? ?? '',
    content: result['content'] as String? ?? '',
    backedUp: result['backedUp'] as bool? ?? false,
    backupPath: result['backupPath'] as String?,
  );
});

// ─── Account capabilities ─────────────────────────────────────────────────────

/// Returns the capability matrix for all registered accounts.
final accountCapabilitiesProvider =
    FutureProvider<List<AccountCapability>>((ref) async {
  final client = ref.read(daemonProvider.notifier).client;
  final result =
      await client.call<Map<String, dynamic>>('account.capabilities', {});

  final rawList = result['capabilities'] as List? ?? [];
  return rawList
      .map((item) =>
          AccountCapability.fromJson(Map<String, dynamic>.from(item as Map)))
      .toList();
});

// ─── Data models ─────────────────────────────────────────────────────────────

/// Status of a single AI provider CLI.
class ProviderStatus {
  const ProviderStatus({
    required this.installed,
    required this.authenticated,
    required this.accountsCount,
    this.version,
    this.path,
  });

  factory ProviderStatus.fromJson(Map<String, dynamic> json) {
    return ProviderStatus(
      installed: json['installed'] as bool? ?? false,
      authenticated: json['authenticated'] as bool? ?? false,
      accountsCount: (json['accountsCount'] as num?)?.toInt() ?? 0,
      version: json['version'] as String?,
      path: json['path'] as String?,
    );
  }

  final bool installed;
  final bool authenticated;
  final int accountsCount;
  final String? version;
  final String? path;
}

/// Answers collected by the 7-question onboarding questionnaire.
class QuestionnaireState {
  const QuestionnaireState({
    this.primaryLanguages = const [],
    this.projectTypes = const [],
    this.teamSize = 'solo',
    this.autonomyLevel = 'balanced',
    this.styleRules = 'strict',
    this.gitWorkflow = 'pr-based',
    this.painPoints = const [],
  });

  final List<String> primaryLanguages;
  final List<String> projectTypes;
  final String teamSize;
  final String autonomyLevel;
  final String styleRules;
  final String gitWorkflow;
  final List<String> painPoints;

  QuestionnaireState copyWith({
    List<String>? primaryLanguages,
    List<String>? projectTypes,
    String? teamSize,
    String? autonomyLevel,
    String? styleRules,
    String? gitWorkflow,
    List<String>? painPoints,
  }) {
    return QuestionnaireState(
      primaryLanguages: primaryLanguages ?? this.primaryLanguages,
      projectTypes: projectTypes ?? this.projectTypes,
      teamSize: teamSize ?? this.teamSize,
      autonomyLevel: autonomyLevel ?? this.autonomyLevel,
      styleRules: styleRules ?? this.styleRules,
      gitWorkflow: gitWorkflow ?? this.gitWorkflow,
      painPoints: painPoints ?? this.painPoints,
    );
  }

  Map<String, dynamic> toJson() => {
        'primaryLanguages': primaryLanguages,
        'projectTypes': projectTypes,
        'teamSize': teamSize,
        'autonomyLevel': autonomyLevel,
        'styleRules': styleRules,
        'gitWorkflow': gitWorkflow,
        'painPoints': painPoints,
      };

  /// Serialise as a canonical string key for the gciPreviewProvider family.
  String toJsonKey() {
    final map = toJson();
    final entries = map.entries.toList()
      ..sort((a, b) => a.key.compareTo(b.key));
    return entries.map((e) => '${e.key}=${e.value}').join('&');
  }

  bool get isComplete =>
      primaryLanguages.isNotEmpty &&
      teamSize.isNotEmpty &&
      autonomyLevel.isNotEmpty;
}

/// Result of calling gci.generate.
class GciPreviewResult {
  const GciPreviewResult({
    required this.path,
    required this.content,
    required this.backedUp,
    this.backupPath,
  });

  final String path;
  final String content;
  final bool backedUp;
  final String? backupPath;
}

/// Per-account capability summary.
class AccountCapability {
  const AccountCapability({
    required this.accountId,
    required this.provider,
    required this.label,
    required this.tier,
    required this.successRate,
    this.rpmLimit,
    this.tpmLimit,
    this.cooldownUntil,
  });

  factory AccountCapability.fromJson(Map<String, dynamic> json) {
    final rateLimits = json['rateLimits'] as Map<String, dynamic>?;
    return AccountCapability(
      accountId: json['accountId'] as String? ?? '',
      provider: json['provider'] as String? ?? '',
      label: json['label'] as String? ?? '',
      tier: json['tier'] as String? ?? 'unknown',
      successRate: (json['successRate'] as num?)?.toDouble() ?? 1.0,
      rpmLimit: rateLimits != null
          ? (rateLimits['rpm'] as num?)?.toInt()
          : null,
      tpmLimit: rateLimits != null
          ? (rateLimits['tpm'] as num?)?.toInt()
          : null,
      cooldownUntil: json['cooldownUntil'] as String?,
    );
  }

  final String accountId;
  final String provider;
  final String label;
  final String tier;
  final double successRate;
  final int? rpmLimit;
  final int? tpmLimit;
  final String? cooldownUntil;

  bool get isCoolingDown {
    if (cooldownUntil == null) return false;
    final until = DateTime.tryParse(cooldownUntil!);
    return until != null && until.isAfter(DateTime.now());
  }

  Duration get remainingCooldown {
    if (!isCoolingDown) return Duration.zero;
    return DateTime.parse(cooldownUntil!).difference(DateTime.now());
  }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Convert a questionnaire JSON key back to a map for RPC calls.
Map<String, dynamic> answersJsonToMap(String jsonKey) {
  // The jsonKey is "key=value&key=value" — we need to reconstruct the map.
  // Since this is used with toJson() output, we parse the structured form.
  // In practice the provider always passes toJson() output directly.
  final parts = jsonKey.split('&');
  final map = <String, dynamic>{};
  for (final part in parts) {
    final idx = part.indexOf('=');
    if (idx < 0) continue;
    final key = part.substring(0, idx);
    final value = part.substring(idx + 1);
    // Reconstruct list values (they appear as "[a, b, c]").
    if (value.startsWith('[') && value.endsWith(']')) {
      final inner = value.substring(1, value.length - 1);
      map[key] = inner.isEmpty
          ? <String>[]
          : inner.split(',').map((s) => s.trim()).toList();
    } else {
      map[key] = value;
    }
  }
  return map;
}
