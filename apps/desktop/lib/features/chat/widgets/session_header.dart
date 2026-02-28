import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

// ME.7 — memory count for a session's repo scope
final _sessionMemoryCountProvider =
    FutureProvider.autoDispose.family<int, String>((ref, repoPath) async {
  final client = ref.read(daemonProvider.notifier).client;
  final result = await client.call('memory.list', {
    'repo_path': repoPath,
    'include_global': true,
  });
  final entries = result['entries'] as List<dynamic>? ?? [];
  return entries.length;
});

class SessionHeader extends ConsumerStatefulWidget {
  const SessionHeader({super.key, required this.session});

  final Session session;

  @override
  ConsumerState<SessionHeader> createState() => _SessionHeaderState();
}

class _SessionHeaderState extends ConsumerState<SessionHeader> {
  String? _currentPhase;

  @override
  void initState() {
    super.initState();
    // UI.1 — listen for task.activityLogged push events to track current phase.
    // The daemon broadcasts phase info in activity log entries.
    ref.listenManual(daemonPushEventsProvider, (_, next) {
      next.whenData((event) {
        final method = event['method'] as String?;
        if (method == 'task.activityLogged') {
          final params = event['params'] as Map<String, dynamic>?;
          final phase = params?['phase'] as String?;
          if (phase != null && mounted) {
            setState(() => _currentPhase = phase);
          }
        }
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    final session = widget.session;
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
    // ME.7 — memory entry count for this session's repo
    final memoryCount =
        ref.watch(_sessionMemoryCountProvider(session.repoPath)).valueOrNull ?? 0;

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
          // UI.1 — phase indicator (shown when a task phase is active)
          if (_currentPhase != null) ...[
            const SizedBox(width: 6),
            PhaseIndicator(
              phase: _currentPhase!,
              isActive: session.status == SessionStatus.running,
            ),
          ],
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
          // ME.7 — memory indicator badge (shown when entries are loaded)
          if (memoryCount > 0) ...[
            const SizedBox(width: 6),
            _MemoryBadge(count: memoryCount),
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
        current: widget.session.modelOverride,
        onSelect: (newModel) {
          ref.read(daemonProvider.notifier).client.setSessionModel(
            widget.session.id,
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
        current: widget.session.mode,
        onSelect: (newMode) {
          ref.read(daemonProvider.notifier).client.setSessionMode(
            widget.session.id,
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

// ME.7 — memory indicator badge
class _MemoryBadge extends StatelessWidget {
  const _MemoryBadge({required this.count});
  final int count;

  @override
  Widget build(BuildContext context) {
    const color = Color(0xFF7C3AED); // purple
    return Tooltip(
      message: '$count memory entries active',
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 3),
        decoration: BoxDecoration(
          color: color.withValues(alpha: 0.12),
          borderRadius: BorderRadius.circular(12),
          border: Border.all(color: color.withValues(alpha: 0.4)),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.memory, size: 11, color: color),
            const SizedBox(width: 3),
            Text(
              '$count',
              style: const TextStyle(
                fontSize: 11,
                color: color,
                fontWeight: FontWeight.w600,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
