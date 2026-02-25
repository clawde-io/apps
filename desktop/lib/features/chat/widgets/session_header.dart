import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

class SessionHeader extends ConsumerWidget {
  const SessionHeader({super.key, required this.session});

  final Session session;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final repoName = session.repoPath.split('/').last;

    // V02.T32 — standards indicator; empty until daemon supports session.standards
    final standards =
        ref.watch(sessionStandardsProvider(session.id)).valueOrNull ?? [];
    // V02.T36 — provider knowledge indicator; empty until daemon supports RPC
    final providerKnowledge =
        ref.watch(sessionProviderKnowledgeProvider(session.id)).valueOrNull ??
            [];
    // SI.T15 — session health indicator; null until daemon supports session.health
    final healthData =
        ref.watch(sessionHealthProvider(session.id)).valueOrNull;

    return Container(
      height: 48,
      padding: const EdgeInsets.symmetric(horizontal: 16),
      decoration: const BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        border: Border(bottom: BorderSide(color: ClawdTheme.surfaceBorder)),
      ),
      child: Row(
        children: [
          Text(
            repoName,
            style: const TextStyle(
              fontWeight: FontWeight.w600,
              fontSize: 14,
            ),
          ),
          const SizedBox(width: 10),
          ProviderBadge(provider: session.provider),
          const SizedBox(width: 10),
          _StatusChip(status: session.status),
          const SizedBox(width: 10),
          // MI.T12 — model override chip + picker
          ModelChip(
            modelOverride: session.modelOverride,
            onTap: () => _showModelPicker(context, ref),
          ),
          if (session.modelOverride != null) const SizedBox(width: 10),
          // V02.T27 / V02.T04 — GCI mode indicator + picker
          ModeBadge(
            mode: session.mode,
            onTap: () => _showModePicker(context, ref),
          ),
          // SI.T15 — session health indicator (hidden when healthy)
          if (healthData != null) ...[
            const SizedBox(width: 6),
            HealthChip(
              healthScore: (healthData['healthScore'] as num?)?.toInt(),
              needsRefresh: healthData['needsRefresh'] as bool? ?? false,
              totalTurns: (healthData['totalTurns'] as num?)?.toInt() ?? 0,
              shortResponseCount:
                  (healthData['shortResponseCount'] as num?)?.toInt() ?? 0,
              toolErrorCount:
                  (healthData['toolErrorCount'] as num?)?.toInt() ?? 0,
              truncationCount:
                  (healthData['truncationCount'] as num?)?.toInt() ?? 0,
            ),
          ],
          if (standards.isNotEmpty) ...[
            const SizedBox(width: 6),
            // V02.T32 — active standards count
            StandardsChip(
              count: standards.length,
              standards: standards,
            ),
          ],
          if (providerKnowledge.isNotEmpty) ...[
            const SizedBox(width: 6),
            // V02.T36 — active provider knowledge count
            ProviderKnowledgeChip(
              count: providerKnowledge.length,
              providers: providerKnowledge,
            ),
          ],
          const Spacer(),
          if (session.status == SessionStatus.running)
            IconButton(
              icon: const Icon(Icons.pause, size: 18),
              tooltip: 'Pause',
              onPressed: () =>
                  ref.read(sessionListProvider.notifier).pause(session.id),
            ),
          if (session.status == SessionStatus.paused)
            IconButton(
              icon: const Icon(Icons.play_arrow, size: 18),
              tooltip: 'Resume',
              onPressed: () =>
                  ref.read(sessionListProvider.notifier).resume(session.id),
            ),
          IconButton(
            icon: const Icon(Icons.close, size: 18),
            tooltip: 'Close',
            onPressed: () async {
              await ref.read(sessionListProvider.notifier).close(session.id);
              ref.read(activeSessionIdProvider.notifier).state = null;
            },
          ),
        ],
      ),
    );
  }

  void _showModelPicker(BuildContext context, WidgetRef ref) {
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: Colors.transparent,
      builder: (_) => ModelPicker(
        current: session.modelOverride,
        onSelect: (newModel) {
          ref.read(daemonProvider.notifier).client.setSessionModel(
            session.id,
            newModel,
          ).ignore();
        },
      ),
    );
  }

  void _showModePicker(BuildContext context, WidgetRef ref) {
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: Colors.transparent,
      builder: (_) => ModePicker(
        current: session.mode,
        onSelect: (newMode) {
          ref.read(daemonProvider.notifier).client.setSessionMode(
            session.id,
            newMode.name.toUpperCase(),
          ).ignore();
        },
      ),
    );
  }
}

class _StatusChip extends StatelessWidget {
  const _StatusChip({required this.status});
  final SessionStatus status;

  (String, Color) get _label => switch (status) {
        SessionStatus.idle => ('Idle', Colors.grey),
        SessionStatus.running => ('Running', ClawdTheme.success),
        SessionStatus.paused => ('Paused', ClawdTheme.warning),
        SessionStatus.completed => ('Done', ClawdTheme.info),
        SessionStatus.error => ('Error', ClawdTheme.error),
      };

  @override
  Widget build(BuildContext context) {
    final (label, color) = _label;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontSize: 11,
          color: color,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}
