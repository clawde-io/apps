import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Desktop task timeline — a vertical list view showing each task as a row
/// with created time, current state badge, duration since creation,
/// and assigned agent badges.
class TaskTimelineScreen extends ConsumerWidget {
  const TaskTimelineScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tasksAsync = ref.watch(taskListProvider(const TaskFilter()));
    final agentsAsync = ref.watch(agentsProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // ── Header ──────────────────────────────────────────────────────────
        Container(
          height: 56,
          padding: const EdgeInsets.symmetric(horizontal: 20),
          decoration: const BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            border: Border(bottom: BorderSide(color: ClawdTheme.surfaceBorder)),
          ),
          child: const Row(
            children: [
              Icon(Icons.timeline, size: 16, color: ClawdTheme.clawLight),
              SizedBox(width: 8),
              Text(
                'Task Timeline',
                style: TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  color: Colors.white,
                ),
              ),
            ],
          ),
        ),

        // ── Timeline list ───────────────────────────────────────────────────
        Expanded(
          child: tasksAsync.when(
            loading: () => const Center(
              child: CircularProgressIndicator(color: ClawdTheme.claw),
            ),
            error: (e, _) => ErrorState(
              icon: Icons.error_outline,
              title: 'Failed to load tasks',
              description: e.toString(),
              onRetry: () => ref.refresh(taskListProvider(const TaskFilter())),
            ),
            data: (tasks) {
              if (tasks.isEmpty) {
                return const EmptyState(
                  icon: Icons.timeline,
                  title: 'No tasks in timeline',
                  subtitle: 'Task activity will appear here once agents begin work.',
                );
              }
              // Sort newest first.
              final sorted = [...tasks]..sort((a, b) {
                  final aTime = a.createdAt ?? '';
                  final bTime = b.createdAt ?? '';
                  return bTime.compareTo(aTime);
                });
              final agents = agentsAsync.valueOrNull ?? [];
              return ListView.builder(
                padding: const EdgeInsets.symmetric(vertical: 8),
                itemCount: sorted.length,
                itemBuilder: (context, i) {
                  final task = sorted[i];
                  final taskAgents = agents
                      .where((a) => a.taskId == task.id)
                      .toList();
                  return _TimelineRow(task: task, agents: taskAgents);
                },
              );
            },
          ),
        ),
      ],
    );
  }
}

// ── Timeline row ───────────────────────────────────────────────────────────────

class _TimelineRow extends StatelessWidget {
  const _TimelineRow({required this.task, required this.agents});
  final AgentTask task;
  final List<AgentRecord> agents;

  String _formatTime(String? iso) {
    if (iso == null) return '—';
    try {
      final dt = DateTime.parse(iso);
      final now = DateTime.now();
      final diff = now.difference(dt);
      if (diff.inSeconds < 60) return '${diff.inSeconds}s ago';
      if (diff.inMinutes < 60) return '${diff.inMinutes}m ago';
      if (diff.inHours < 24) return '${diff.inHours}h ago';
      return '${diff.inDays}d ago';
    } catch (_) {
      return iso;
    }
  }

  String _duration(String? iso) {
    if (iso == null) return '';
    try {
      final created = DateTime.parse(iso);
      final diff = DateTime.now().difference(created);
      if (diff.inSeconds < 60) return '${diff.inSeconds}s';
      if (diff.inMinutes < 60) return '${diff.inMinutes}m';
      return '${diff.inHours}h ${diff.inMinutes % 60}m';
    } catch (_) {
      return '';
    }
  }

  @override
  Widget build(BuildContext context) {
    return IntrinsicHeight(
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // ── Timeline line + dot ────────────────────────────────────────────
          SizedBox(
            width: 48,
            child: Column(
              children: [
                Container(
                  width: 2,
                  height: 12,
                  color: ClawdTheme.surfaceBorder,
                ),
                Container(
                  width: 10,
                  height: 10,
                  decoration: BoxDecoration(
                    color: ClawdTheme.claw,
                    shape: BoxShape.circle,
                    border: Border.all(
                      color: ClawdTheme.clawLight,
                      width: 2,
                    ),
                  ),
                ),
                Expanded(
                  child: Container(
                    width: 2,
                    color: ClawdTheme.surfaceBorder,
                  ),
                ),
              ],
            ),
          ),

          // ── Task card ──────────────────────────────────────────────────────
          Expanded(
            child: Container(
              margin: const EdgeInsets.only(right: 16, bottom: 4, top: 4),
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: ClawdTheme.surfaceElevated,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: ClawdTheme.surfaceBorder),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  // Top row: title + status badge
                  Row(
                    children: [
                      Expanded(
                        child: Text(
                          task.title,
                          style: const TextStyle(
                            fontSize: 12,
                            fontWeight: FontWeight.w600,
                            color: Colors.white,
                          ),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      const SizedBox(width: 8),
                      TaskStatusBadge(status: task.status),
                    ],
                  ),
                  const SizedBox(height: 4),

                  // Time + duration row
                  Row(
                    children: [
                      const Icon(Icons.schedule, size: 11, color: Colors.white38),
                      const SizedBox(width: 4),
                      Text(
                        _formatTime(task.createdAt),
                        style: const TextStyle(
                            fontSize: 11, color: Colors.white38),
                      ),
                      if (task.createdAt != null) ...[
                        const SizedBox(width: 8),
                        const Icon(Icons.timer_outlined,
                            size: 11, color: Colors.white38),
                        const SizedBox(width: 4),
                        Text(
                          _duration(task.createdAt),
                          style: const TextStyle(
                              fontSize: 11, color: Colors.white38),
                        ),
                      ],
                    ],
                  ),

                  // Agent badges
                  if (agents.isNotEmpty) ...[
                    const SizedBox(height: 6),
                    Wrap(
                      spacing: 6,
                      runSpacing: 4,
                      children: agents
                          .map((a) => _AgentBadge(agent: a))
                          .toList(),
                    ),
                  ],
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _AgentBadge extends StatelessWidget {
  const _AgentBadge({required this.agent});
  final AgentRecord agent;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
      decoration: BoxDecoration(
        color: ClawdTheme.claw.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        agent.role.displayName,
        style: const TextStyle(
          fontSize: 10,
          color: ClawdTheme.clawLight,
        ),
      ),
    );
  }
}
