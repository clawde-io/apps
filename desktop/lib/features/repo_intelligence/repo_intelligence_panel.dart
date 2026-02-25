/// Repo Intelligence panel — shows the detected stack profile, artifact status,
/// drift score, and "Generate AI configs" action (RI.T19–T20, Sprint F).
///
/// This is a stub implementation wired to the daemon's new RPCs.
/// Full UI polish ships in Sprint U (UI.T19).
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/features/repo/repo_context_provider.dart';
import 'package:clawde/features/repo_intelligence/repo_intelligence_provider.dart';

/// Inline panel shown at the bottom of the Files tab left pane.
class RepoIntelligencePanel extends ConsumerWidget {
  const RepoIntelligencePanel({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final repoPath = ref.watch(effectiveRepoPathProvider);
    if (repoPath == null) return const SizedBox.shrink();

    final profileAsync = ref.watch(repoProfileProvider(repoPath));

    return Container(
      decoration: const BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        border: Border(top: BorderSide(color: ClawdTheme.surfaceBorder)),
      ),
      padding: const EdgeInsets.all(12),
      child: profileAsync.when(
        loading: () => const _PanelLoading(),
        error: (e, _) => _PanelError(error: e.toString()),
        data: (profile) => profile == null
            ? _PanelEmpty(repoPath: repoPath)
            : _PanelContent(profile: profile, repoPath: repoPath),
      ),
    );
  }
}

// ─── Loading / Error / Empty ─────────────────────────────────────────────────

class _PanelLoading extends StatelessWidget {
  const _PanelLoading();

  @override
  Widget build(BuildContext context) => const Padding(
        padding: EdgeInsets.symmetric(vertical: 8),
        child: Row(
          children: [
            SizedBox(
              width: 12,
              height: 12,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
            SizedBox(width: 8),
            Text('Scanning repo…',
                style: TextStyle(fontSize: 11, color: Colors.white54)),
          ],
        ),
      );
}

class _PanelError extends StatelessWidget {
  const _PanelError({required this.error});
  final String error;

  @override
  Widget build(BuildContext context) => Text(
        'Scan failed: $error',
        style: const TextStyle(fontSize: 11, color: Colors.redAccent),
        maxLines: 2,
        overflow: TextOverflow.ellipsis,
      );
}

class _PanelEmpty extends ConsumerWidget {
  const _PanelEmpty({required this.repoPath});
  final String repoPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Repo Intelligence',
            style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.bold,
                color: Colors.white70),
          ),
          const SizedBox(height: 6),
          const Text(
            'Not yet scanned.',
            style: TextStyle(fontSize: 11, color: Colors.white54),
          ),
          const SizedBox(height: 8),
          _ScanButton(repoPath: repoPath),
        ],
      );
}

// ─── Main panel content ───────────────────────────────────────────────────────

class _PanelContent extends ConsumerWidget {
  const _PanelContent({required this.profile, required this.repoPath});
  final Map<String, dynamic> profile;
  final String repoPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final driftAsync = ref.watch(repoDriftScoreProvider(repoPath));
    final lang = profile['primaryLang'] as String? ?? 'unknown';
    final frameworks =
        (profile['frameworks'] as List?)?.cast<String>() ?? [];
    final confidence =
        ((profile['confidence'] as num?)?.toDouble() ?? 0.0) * 100;
    final monorepo = profile['monorepo'] as bool? ?? false;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // Header
        Row(
          children: [
            const Icon(Icons.auto_awesome_outlined,
                size: 12, color: ClawdTheme.claw),
            const SizedBox(width: 6),
            const Text(
              'Repo Intelligence',
              style: TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.bold,
                  color: Colors.white70),
            ),
            const Spacer(),
            _ScanButton(repoPath: repoPath, compact: true),
          ],
        ),
        const SizedBox(height: 8),

        // Stack chips
        Wrap(
          spacing: 4,
          runSpacing: 4,
          children: [
            _Chip(label: lang, icon: Icons.code_outlined),
            if (monorepo) const _Chip(label: 'monorepo'),
            ...frameworks.take(3).map((f) => _Chip(label: f)),
          ],
        ),
        const SizedBox(height: 6),

        // Confidence
        Text(
          'Confidence: ${confidence.toStringAsFixed(0)}%',
          style: const TextStyle(fontSize: 10, color: Colors.white38),
        ),
        const SizedBox(height: 4),

        // Drift score
        driftAsync.when(
          loading: () => const SizedBox.shrink(),
          error: (_, __) => const SizedBox.shrink(),
          data: (score) => _DriftBar(score: score),
        ),

        const SizedBox(height: 8),

        // Generate AI configs button (RI.T20)
        _GenerateArtifactsButton(repoPath: repoPath),
      ],
    );
  }
}

// ─── Sub-widgets ──────────────────────────────────────────────────────────────

class _Chip extends StatelessWidget {
  const _Chip({required this.label, this.icon});
  final String label;
  final IconData? icon;

  @override
  Widget build(BuildContext context) => Container(
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
        decoration: BoxDecoration(
          color: ClawdTheme.surfaceBorder,
          borderRadius: BorderRadius.circular(4),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (icon != null) ...[
              Icon(icon, size: 9, color: Colors.white60),
              const SizedBox(width: 3),
            ],
            Text(
              label,
              style: const TextStyle(fontSize: 10, color: Colors.white70),
            ),
          ],
        ),
      );
}

class _DriftBar extends StatelessWidget {
  const _DriftBar({required this.score});
  final int score;

  Color get _color {
    if (score >= 80) return Colors.greenAccent;
    if (score >= 50) return Colors.orangeAccent;
    return Colors.redAccent;
  }

  @override
  Widget build(BuildContext context) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Artifact sync: $score/100',
            style: TextStyle(fontSize: 10, color: _color),
          ),
          const SizedBox(height: 2),
          ClipRRect(
            borderRadius: BorderRadius.circular(2),
            child: LinearProgressIndicator(
              value: score / 100.0,
              backgroundColor: ClawdTheme.surfaceBorder,
              valueColor: AlwaysStoppedAnimation<Color>(_color),
              minHeight: 3,
            ),
          ),
        ],
      );
}

class _ScanButton extends ConsumerWidget {
  const _ScanButton({required this.repoPath, this.compact = false});
  final String repoPath;
  final bool compact;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return TextButton(
      style: TextButton.styleFrom(
        padding: compact
            ? const EdgeInsets.symmetric(horizontal: 6, vertical: 2)
            : const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
        minimumSize: Size.zero,
        tapTargetSize: MaterialTapTargetSize.shrinkWrap,
      ),
      onPressed: () => ref.read(repoScanActionsProvider).scan(repoPath),
      child: Text(
        compact ? 'Rescan' : 'Scan repo',
        style: const TextStyle(fontSize: 11, color: ClawdTheme.claw),
      ),
    );
  }
}

/// "Generate AI configs" action button (RI.T20).
///
/// Calls `repo.generateArtifacts` with `overwrite: false`, then shows
/// a diff dialog for each artifact so the user can approve or skip per-file.
class _GenerateArtifactsButton extends ConsumerWidget {
  const _GenerateArtifactsButton({required this.repoPath});
  final String repoPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) => SizedBox(
        width: double.infinity,
        child: OutlinedButton.icon(
          style: OutlinedButton.styleFrom(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            side: const BorderSide(color: ClawdTheme.surfaceBorder),
            minimumSize: Size.zero,
            tapTargetSize: MaterialTapTargetSize.shrinkWrap,
          ),
          icon: const Icon(Icons.auto_fix_high_outlined,
              size: 13, color: Colors.white60),
          label: const Text(
            'Generate AI configs',
            style: TextStyle(fontSize: 11, color: Colors.white70),
          ),
          onPressed: () =>
              _onGenerate(context, ref),
        ),
      );

  Future<void> _onGenerate(BuildContext context, WidgetRef ref) async {
    final actions = ref.read(repoScanActionsProvider);
    final results = await actions.generateArtifacts(repoPath, overwrite: false);

    if (!context.mounted) return;

    // Show per-artifact result summary
    final created = results.where((r) => r['action'] == 'created').length;
    final updated = results.where((r) => r['action'] == 'updated').length;
    final skipped = results.where((r) => r['action'] == 'skipped').length;
    final msg = '$created created, $updated updated, $skipped skipped';

    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text('AI configs: $msg'),
        duration: const Duration(seconds: 3),
      ),
    );
  }
}
