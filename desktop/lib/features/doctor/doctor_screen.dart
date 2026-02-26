// SPDX-License-Identifier: MIT
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:clawde/features/repo/repo_context_provider.dart';

// ─── Doctor Screen ────────────────────────────────────────────────────────────

/// Project health scanner — consumes `doctor.scan` / `doctor.fix` RPCs.
///
/// Displays grouped findings (AFS, Docs, Release) with severity icons,
/// an overall health score, and per-group fix buttons.
class DoctorScreen extends ConsumerWidget {
  const DoctorScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final projectPath = ref.watch(effectiveRepoPathProvider);

    return Column(
      children: [
        // ── Header ─────────────────────────────────────────────────────────
        _Header(projectPath: projectPath),

        // ── Body ───────────────────────────────────────────────────────────
        Expanded(
          child: projectPath == null
              ? const _NoProjectPlaceholder()
              : _DoctorBody(projectPath: projectPath),
        ),
      ],
    );
  }
}

// ─── Header ───────────────────────────────────────────────────────────────────

class _Header extends ConsumerWidget {
  const _Header({required this.projectPath});

  final String? projectPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scoreAsync = projectPath == null
        ? const AsyncValue<int?>.data(null)
        : ref.watch(
            doctorProvider(projectPath!).select(
              (v) => v.whenData((r) => r?.score),
            ),
          );

    return Container(
      height: 56,
      padding: const EdgeInsets.symmetric(horizontal: 20),
      decoration: const BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        border: Border(bottom: BorderSide(color: ClawdTheme.surfaceBorder)),
      ),
      child: Row(
        children: [
          const Text(
            'Doctor',
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w700,
              color: Colors.white,
            ),
          ),
          const SizedBox(width: 10),
          scoreAsync.when(
            loading: () => const SizedBox(
              width: 14,
              height: 14,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
            error: (_, __) => const SizedBox.shrink(),
            data: (score) =>
                score == null ? const SizedBox.shrink() : _ScoreBadge(score),
          ),
          const Spacer(),
          if (projectPath != null)
            _RunButton(projectPath: projectPath!),
        ],
      ),
    );
  }
}

// ─── Score Badge ──────────────────────────────────────────────────────────────

class _ScoreBadge extends StatelessWidget {
  const _ScoreBadge(this.score);

  final int score;

  Color get _color {
    if (score >= 90) return const Color(0xFF22c55e);
    if (score >= 70) return ClawdTheme.warning;
    return ClawdTheme.claw;
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      decoration: BoxDecoration(
        color: _color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: _color.withValues(alpha: 0.4)),
      ),
      child: Text(
        'Score $score',
        style: TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w600,
          color: _color,
        ),
      ),
    );
  }
}

// ─── Run Button ───────────────────────────────────────────────────────────────

class _RunButton extends ConsumerWidget {
  const _RunButton({required this.projectPath});

  final String projectPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final isLoading =
        ref.watch(doctorProvider(projectPath).select((v) => v.isLoading));

    return FilledButton.icon(
      onPressed: isLoading
          ? null
          : () => ref.read(doctorProvider(projectPath).notifier).scan(),
      icon: isLoading
          ? const SizedBox(
              width: 14,
              height: 14,
              child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white),
            )
          : const Icon(Icons.health_and_safety_outlined, size: 16),
      label: const Text('Run Checks', style: TextStyle(fontSize: 12)),
      style: FilledButton.styleFrom(
        backgroundColor: ClawdTheme.claw,
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
      ),
    );
  }
}

// ─── No Project Placeholder ───────────────────────────────────────────────────

class _NoProjectPlaceholder extends StatelessWidget {
  const _NoProjectPlaceholder();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.health_and_safety_outlined,
            size: 48,
            color: Colors.white.withValues(alpha: 0.2),
          ),
          const SizedBox(height: 16),
          Text(
            'No project open',
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w600,
              color: Colors.white.withValues(alpha: 0.4),
            ),
          ),
          const SizedBox(height: 8),
          Text(
            'Open a session with a repo path to run Doctor checks.',
            style: TextStyle(
              fontSize: 12,
              color: Colors.white.withValues(alpha: 0.3),
            ),
          ),
        ],
      ),
    );
  }
}

// ─── Doctor Body ─────────────────────────────────────────────────────────────

class _DoctorBody extends ConsumerWidget {
  const _DoctorBody({required this.projectPath});

  final String projectPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final doctorAsync = ref.watch(doctorProvider(projectPath));

    return doctorAsync.when(
      loading: () => const Center(
        child: CircularProgressIndicator(strokeWidth: 2),
      ),
      error: (e, _) => Center(
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(Icons.error_outline, size: 36, color: Colors.red),
              const SizedBox(height: 12),
              Text(
                'Scan failed: $e',
                style: const TextStyle(fontSize: 12, color: Colors.white54),
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      ),
      data: (result) {
        if (result == null) return const _NotScannedPlaceholder();
        return _FindingsView(projectPath: projectPath, result: result);
      },
    );
  }
}

// ─── Not Scanned Placeholder ──────────────────────────────────────────────────

class _NotScannedPlaceholder extends StatelessWidget {
  const _NotScannedPlaceholder();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.health_and_safety_outlined,
            size: 48,
            color: Colors.white.withValues(alpha: 0.2),
          ),
          const SizedBox(height: 16),
          Text(
            'Run Checks to scan your project',
            style: TextStyle(
              fontSize: 14,
              color: Colors.white.withValues(alpha: 0.4),
            ),
          ),
          const SizedBox(height: 8),
          Text(
            'Doctor checks AFS structure, docs, and release readiness.',
            style: TextStyle(
              fontSize: 12,
              color: Colors.white.withValues(alpha: 0.25),
            ),
          ),
        ],
      ),
    );
  }
}

// ─── Findings View ────────────────────────────────────────────────────────────

class _FindingsView extends ConsumerWidget {
  const _FindingsView({required this.projectPath, required this.result});

  final String projectPath;
  final DoctorScanResult result;

  static const _groups = ['afs', 'docs', 'release'];
  static const _groupLabels = {'afs': 'AFS', 'docs': 'Docs', 'release': 'Release'};
  static const _groupIcons = {
    'afs': Icons.folder_special_outlined,
    'docs': Icons.description_outlined,
    'release': Icons.rocket_launch_outlined,
  };

  List<DoctorFinding> _findingsFor(String group) => result.findings
      .where((f) => f.code.startsWith(group))
      .toList();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // ── Summary row ──────────────────────────────────────────────────
          _SummaryRow(result: result),
          const SizedBox(height: 24),

          // ── Group sections ───────────────────────────────────────────────
          for (final group in _groups) ...[
            _GroupSection(
              projectPath: projectPath,
              group: group,
              label: _groupLabels[group]!,
              icon: _groupIcons[group]!,
              findings: _findingsFor(group),
            ),
            const SizedBox(height: 16),
          ],
        ],
      ),
    );
  }
}

// ─── Summary Row ─────────────────────────────────────────────────────────────

class _SummaryRow extends StatelessWidget {
  const _SummaryRow({required this.result});

  final DoctorScanResult result;

  int _count(DoctorSeverity sev) =>
      result.findings.where((f) => f.severity == sev).length;

  @override
  Widget build(BuildContext context) {
    final criticalCount = _count(DoctorSeverity.critical);
    final highCount = _count(DoctorSeverity.high);
    final medCount = _count(DoctorSeverity.medium);
    final fixable = result.findings.where((f) => f.fixable).length;

    return Row(
      children: [
        _StatCard(
          label: 'Critical',
          value: '$criticalCount',
          color: const Color(0xFFef4444),
        ),
        const SizedBox(width: 10),
        _StatCard(
          label: 'High',
          value: '$highCount',
          color: const Color(0xFFf97316),
        ),
        const SizedBox(width: 10),
        _StatCard(
          label: 'Medium',
          value: '$medCount',
          color: ClawdTheme.warning,
        ),
        const SizedBox(width: 10),
        _StatCard(
          label: 'Auto-fixable',
          value: '$fixable',
          color: const Color(0xFF22c55e),
        ),
      ],
    );
  }
}

class _StatCard extends StatelessWidget {
  const _StatCard({required this.label, required this.value, required this.color});

  final String label;
  final String value;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Column(
        children: [
          Text(
            value,
            style: TextStyle(
              fontSize: 20,
              fontWeight: FontWeight.w700,
              color: color,
            ),
          ),
          const SizedBox(height: 2),
          Text(
            label,
            style: const TextStyle(fontSize: 10, color: Colors.white38),
          ),
        ],
      ),
    );
  }
}

// ─── Group Section ────────────────────────────────────────────────────────────

class _GroupSection extends ConsumerWidget {
  const _GroupSection({
    required this.projectPath,
    required this.group,
    required this.label,
    required this.icon,
    required this.findings,
  });

  final String projectPath;
  final String group;
  final String label;
  final IconData icon;
  final List<DoctorFinding> findings;

  bool get _hasIssues => findings.isNotEmpty;
  bool get _hasFixable => findings.any((f) => f.fixable);

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Section header
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 12, 12),
            child: Row(
              children: [
                Icon(icon, size: 16, color: Colors.white54),
                const SizedBox(width: 8),
                Text(
                  label,
                  style: const TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    color: Colors.white,
                  ),
                ),
                const SizedBox(width: 8),
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
                  decoration: BoxDecoration(
                    color: _hasIssues
                        ? ClawdTheme.claw.withValues(alpha: 0.2)
                        : const Color(0xFF22c55e).withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Text(
                    _hasIssues ? '${findings.length}' : '✓',
                    style: TextStyle(
                      fontSize: 10,
                      fontWeight: FontWeight.w600,
                      color: _hasIssues
                          ? ClawdTheme.clawLight
                          : const Color(0xFF22c55e),
                    ),
                  ),
                ),
                const Spacer(),
                if (_hasFixable)
                  _FixGroupButton(
                    projectPath: projectPath,
                    group: group,
                    findings: findings,
                  ),
              ],
            ),
          ),

          // Findings
          if (_hasIssues) ...[
            const Divider(height: 1, color: ClawdTheme.surfaceBorder),
            ...findings.map((f) => _FindingTile(finding: f)),
          ],
        ],
      ),
    );
  }
}

// ─── Fix Group Button ─────────────────────────────────────────────────────────

class _FixGroupButton extends ConsumerWidget {
  const _FixGroupButton({
    required this.projectPath,
    required this.group,
    required this.findings,
  });

  final String projectPath;
  final String group;
  final List<DoctorFinding> findings;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final isLoading =
        ref.watch(doctorProvider(projectPath).select((v) => v.isLoading));

    final fixableCodes =
        findings.where((f) => f.fixable).map((f) => f.code).toList();

    return TextButton.icon(
      onPressed: isLoading || fixableCodes.isEmpty
          ? null
          : () => ref
              .read(doctorProvider(projectPath).notifier)
              .fix(codes: fixableCodes),
      icon: const Icon(Icons.auto_fix_high, size: 13),
      label: const Text('Fix', style: TextStyle(fontSize: 11)),
      style: TextButton.styleFrom(
        foregroundColor: const Color(0xFF22c55e),
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
      ),
    );
  }
}

// ─── Finding Tile ─────────────────────────────────────────────────────────────

class _FindingTile extends StatelessWidget {
  const _FindingTile({required this.finding});

  final DoctorFinding finding;

  Color get _severityColor => switch (finding.severity) {
        DoctorSeverity.critical => const Color(0xFFef4444),
        DoctorSeverity.high => const Color(0xFFf97316),
        DoctorSeverity.medium => ClawdTheme.warning,
        DoctorSeverity.low => const Color(0xFF60a5fa),
        DoctorSeverity.info => Colors.white38,
      };

  IconData get _severityIcon => switch (finding.severity) {
        DoctorSeverity.critical => Icons.error,
        DoctorSeverity.high => Icons.warning,
        DoctorSeverity.medium => Icons.warning_amber_outlined,
        DoctorSeverity.low => Icons.info_outline,
        DoctorSeverity.info => Icons.info_outline,
      };

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 10, 16, 10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 1),
            child: Icon(_severityIcon, size: 14, color: _severityColor),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  finding.message,
                  style: const TextStyle(fontSize: 12, color: Colors.white),
                ),
                if (finding.path != null) ...[
                  const SizedBox(height: 3),
                  Text(
                    finding.path!,
                    style: const TextStyle(fontSize: 10, color: Colors.white38),
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(width: 8),
          // Severity + fixable badges
          Row(
            children: [
              Container(
                padding:
                    const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                decoration: BoxDecoration(
                  color: _severityColor.withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  finding.severity.name,
                  style: TextStyle(
                    fontSize: 9,
                    fontWeight: FontWeight.w600,
                    color: _severityColor,
                  ),
                ),
              ),
              if (finding.fixable) ...[
                const SizedBox(width: 4),
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                  decoration: BoxDecoration(
                    color: const Color(0xFF22c55e).withValues(alpha: 0.12),
                    borderRadius: BorderRadius.circular(4),
                  ),
                  child: const Text(
                    'fixable',
                    style: TextStyle(
                      fontSize: 9,
                      fontWeight: FontWeight.w600,
                      color: Color(0xFF22c55e),
                    ),
                  ),
                ),
              ],
            ],
          ),
        ],
      ),
    );
  }
}
