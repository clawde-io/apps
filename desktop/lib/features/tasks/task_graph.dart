import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Task Graph view — a ListView-based DAG representation showing tasks
/// grouped by status with colour-coded badges and dependency indentation.
class TaskGraphScreen extends ConsumerWidget {
  const TaskGraphScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tasksAsync = ref.watch(
      taskListProvider(const TaskFilter()),
    );
    final agentsAsync = ref.watch(agentsProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // ── Header ──────────────────────────────────────────────────────────
        _TaskGraphHeader(tasksAsync: tasksAsync),

        // ── Task list ────────────────────────────────────────────────────────
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
                  icon: Icons.account_tree_outlined,
                  title: 'No tasks yet',
                  subtitle: 'Tasks will appear here once agents begin work.',
                );
              }

              final agents = agentsAsync.valueOrNull ?? [];
              // Group by status for visual separation.
              final groups = _groupByStatus(tasks);

              return ListView.builder(
                padding: const EdgeInsets.symmetric(vertical: 8),
                itemCount: groups.entries.length,
                itemBuilder: (context, groupIdx) {
                  final entry = groups.entries.elementAt(groupIdx);
                  return _TaskGroup(
                    status: entry.key,
                    tasks: entry.value,
                    agents: agents,
                  );
                },
              );
            },
          ),
        ),
      ],
    );
  }

  /// Orders status groups for display: running → pending → in_qa → blocked → done → deferred.
  Map<TaskStatus, List<AgentTask>> _groupByStatus(List<AgentTask> tasks) {
    const order = [
      TaskStatus.inProgress,
      TaskStatus.pending,
      TaskStatus.inQa,
      TaskStatus.blocked,
      TaskStatus.interrupted,
      TaskStatus.done,
      TaskStatus.deferred,
    ];
    final map = <TaskStatus, List<AgentTask>>{};
    for (final status in order) {
      final group = tasks.where((t) => t.status == status).toList();
      if (group.isNotEmpty) map[status] = group;
    }
    return map;
  }
}

// ── Header ─────────────────────────────────────────────────────────────────────

class _TaskGraphHeader extends StatelessWidget {
  const _TaskGraphHeader({required this.tasksAsync});
  final AsyncValue<List<AgentTask>> tasksAsync;

  @override
  Widget build(BuildContext context) {
    final count = tasksAsync.valueOrNull?.length ?? 0;
    return Container(
      height: 56,
      padding: const EdgeInsets.symmetric(horizontal: 20),
      decoration: const BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        border: Border(bottom: BorderSide(color: ClawdTheme.surfaceBorder)),
      ),
      child: Row(
        children: [
          const Icon(Icons.account_tree_outlined, size: 16, color: ClawdTheme.clawLight),
          const SizedBox(width: 8),
          const Text(
            'Task Graph',
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w700,
              color: Colors.white,
            ),
          ),
          const SizedBox(width: 8),
          if (tasksAsync.hasValue)
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
              decoration: BoxDecoration(
                color: ClawdTheme.claw.withValues(alpha: 0.2),
                borderRadius: BorderRadius.circular(10),
              ),
              child: Text(
                '$count',
                style: const TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.w600,
                  color: ClawdTheme.clawLight,
                ),
              ),
            ),
        ],
      ),
    );
  }
}

// ── Task group ─────────────────────────────────────────────────────────────────

class _TaskGroup extends StatelessWidget {
  const _TaskGroup({
    required this.status,
    required this.tasks,
    required this.agents,
  });
  final TaskStatus status;
  final List<AgentTask> tasks;
  final List<AgentRecord> agents;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // Group header
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 4),
          child: Row(
            children: [
              TaskStatusBadge(status: status),
              const SizedBox(width: 8),
              Text(
                '${tasks.length}',
                style: const TextStyle(fontSize: 11, color: Colors.white38),
              ),
            ],
          ),
        ),
        // Tasks
        ...tasks.map((task) {
          final assignedAgent = agents
              .where((a) => a.taskId == task.id)
              .firstOrNull;
          return _TaskGraphRow(task: task, assignedAgent: assignedAgent);
        }),
      ],
    );
  }
}

// ── Task row ───────────────────────────────────────────────────────────────────

class _TaskGraphRow extends StatelessWidget {
  const _TaskGraphRow({required this.task, this.assignedAgent});
  final AgentTask task;
  final AgentRecord? assignedAgent;

  bool get _hasWorktree => task.repoPath != null;

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 3),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Row(
        children: [
          // Task ID + title
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(
                      task.id,
                      style: const TextStyle(
                        fontSize: 10,
                        fontFamily: 'monospace',
                        color: Colors.white38,
                      ),
                    ),
                    if (_hasWorktree) ...[
                      const SizedBox(width: 6),
                      const Icon(
                        Icons.call_split,
                        size: 11,
                        color: ClawdTheme.clawLight,
                      ),
                    ],
                  ],
                ),
                const SizedBox(height: 2),
                Text(
                  task.title,
                  style: const TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                    color: Colors.white,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ],
            ),
          ),
          const SizedBox(width: 8),

          // Assigned agent chip
          if (assignedAgent != null) ...[
            _AgentRoleChip(role: assignedAgent!.role),
            const SizedBox(width: 8),
          ],

          // Status badge
          TaskStatusBadge(status: task.status),
        ],
      ),
    );
  }
}

class _AgentRoleChip extends StatelessWidget {
  const _AgentRoleChip({required this.role});
  final AgentRole role;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: ClawdTheme.claw.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        role.displayName,
        style: const TextStyle(
          fontSize: 10,
          fontWeight: FontWeight.w600,
          color: ClawdTheme.clawLight,
        ),
      ),
    );
  }
}
