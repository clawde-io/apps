import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

class AgentDashboardScreen extends ConsumerStatefulWidget {
  const AgentDashboardScreen({super.key});

  @override
  ConsumerState<AgentDashboardScreen> createState() =>
      _AgentDashboardScreenState();
}

class _AgentDashboardScreenState
    extends ConsumerState<AgentDashboardScreen> {
  bool _showActivity = true;

  @override
  Widget build(BuildContext context) {
    final repoPath = ref.watch(activeRepoPathProvider);
    final filter = ref.watch(dashboardFilterProvider);
    final byStatus = ref.watch(tasksByStatusProvider(repoPath));
    final selectedTask = ref.watch(selectedTaskProvider);
    final summaryAsync = ref.watch(taskSummaryProvider(repoPath));
    final activityAsync = ref.watch(activityFeedProvider(repoPath));
    final agentsAsync = ref.watch(agentListProvider(repoPath));

    return Column(
      children: [
        // ── Header ─────────────────────────────────────────────────────────
        _DashboardHeader(
          repoPath: repoPath,
          summaryAsync: summaryAsync,
          agentsAsync: agentsAsync,
          filter: filter,
          showActivity: _showActivity,
          onToggleActivity: () =>
              setState(() => _showActivity = !_showActivity),
        ),

        // ── Main body ──────────────────────────────────────────────────────
        Expanded(
          child: Row(
            children: [
              // ── Kanban board ─────────────────────────────────────────────
              Expanded(
                child: Column(
                  children: [
                    if (filter.isActive)
                      _FilterBar(
                        filter: filter,
                        onClear: () => ref
                            .read(dashboardFilterProvider.notifier)
                            .state = const DashboardFilter(),
                      ),
                    Expanded(
                      child: KanbanBoard(
                        tasksByStatus: byStatus,
                        onTaskTap: (task) {
                          ref.read(selectedTaskIdProvider.notifier).state =
                              task.id;
                        },
                      ),
                    ),
                  ],
                ),
              ),

              // ── Right panel: task detail OR activity feed ─────────────
              if (selectedTask != null)
                SizedBox(
                  width: 320,
                  child: Column(
                    children: [
                      const VerticalDivider(thickness: 1, width: 1),
                      Expanded(
                        child: TaskDetailPanel(
                          task: selectedTask,
                          onClose: () {
                            ref
                                .read(selectedTaskIdProvider.notifier)
                                .state = null;
                          },
                          onMarkDone: (notes) async {
                            final client = ref
                                .read(daemonProvider.notifier)
                                .client;
                            await client.updateTaskStatus(
                              selectedTask.id,
                              'done',
                              notes: notes.isEmpty ? null : notes,
                            );
                          },
                          onMarkBlocked: () async {
                            final client = ref
                                .read(daemonProvider.notifier)
                                .client;
                            await client.updateTaskStatus(
                              selectedTask.id,
                              'blocked',
                            );
                          },
                        ),
                      ),
                    ],
                  ),
                )
              else if (_showActivity)
                SizedBox(
                  width: 300,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const VerticalDivider(thickness: 1, width: 1),
                      Container(
                        height: 40,
                        padding: const EdgeInsets.symmetric(horizontal: 16),
                        decoration: const BoxDecoration(
                          color: ClawdTheme.surfaceElevated,
                          border: Border(
                            bottom: BorderSide(color: ClawdTheme.surfaceBorder),
                          ),
                        ),
                        child: const Align(
                          alignment: Alignment.centerLeft,
                          child: Text(
                            'Activity',
                            style: TextStyle(
                              fontSize: 12,
                              fontWeight: FontWeight.w600,
                              color: Colors.white70,
                            ),
                          ),
                        ),
                      ),
                      Expanded(
                        child: activityAsync.when(
                          loading: () => const Center(
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                          error: (e, _) => Center(
                            child: Text(
                              'Failed to load activity\n$e',
                              style: const TextStyle(
                                fontSize: 11,
                                color: Colors.white38,
                              ),
                              textAlign: TextAlign.center,
                            ),
                          ),
                          data: (entries) => entries.isEmpty
                              ? const Center(
                                  child: Text(
                                    'No activity yet',
                                    style: TextStyle(
                                      fontSize: 12,
                                      color: Colors.white38,
                                    ),
                                  ),
                                )
                              : ActivityFeed(entries: entries),
                        ),
                      ),
                    ],
                  ),
                ),
            ],
          ),
        ),
      ],
    );
  }
}

// ── Header ────────────────────────────────────────────────────────────────────

class _DashboardHeader extends ConsumerWidget {
  const _DashboardHeader({
    required this.repoPath,
    required this.summaryAsync,
    required this.agentsAsync,
    required this.filter,
    required this.showActivity,
    required this.onToggleActivity,
  });

  final String? repoPath;
  final AsyncValue<TaskSummary> summaryAsync;
  final AsyncValue<List<AgentView>> agentsAsync;
  final DashboardFilter filter;
  final bool showActivity;
  final VoidCallback onToggleActivity;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final total = summaryAsync.valueOrNull?.total ?? 0;
    final inProgress =
        summaryAsync.valueOrNull?.byStatus['in_progress'] ?? 0;
    final agents = agentsAsync.valueOrNull ?? [];

    return Container(
      height: 56,
      padding: const EdgeInsets.symmetric(horizontal: 20),
      decoration: const BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        border: Border(
          bottom: BorderSide(color: ClawdTheme.surfaceBorder),
        ),
      ),
      child: Row(
        children: [
          const Text(
            'Tasks',
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w700,
              color: Colors.white,
            ),
          ),
          const SizedBox(width: 8),
          if (total > 0)
            Container(
              padding:
                  const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
              decoration: BoxDecoration(
                color: ClawdTheme.claw.withValues(alpha: 0.2),
                borderRadius: BorderRadius.circular(10),
              ),
              child: Text(
                '$total',
                style: const TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.w600,
                  color: ClawdTheme.clawLight,
                ),
              ),
            ),
          if (inProgress > 0) ...[
            const SizedBox(width: 8),
            Container(
              padding:
                  const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
              decoration: BoxDecoration(
                color: Colors.green.withValues(alpha: 0.15),
                borderRadius: BorderRadius.circular(10),
              ),
              child: Text(
                '$inProgress active',
                style: const TextStyle(
                  fontSize: 11,
                  color: Colors.green,
                ),
              ),
            ),
          ],

          // Agent chips
          if (agents.isNotEmpty) ...[
            const SizedBox(width: 12),
            const VerticalDivider(width: 1, thickness: 1),
            const SizedBox(width: 12),
            Wrap(
              spacing: 6,
              children: agents
                  .take(4)
                  .map((a) => AgentChip(
                        agentId: a.agentId,
                        isActive: a.status == AgentStatus.active,
                      ))
                  .toList(),
            ),
            if (agents.length > 4)
              Text(
                '+${agents.length - 4} more',
                style: const TextStyle(
                  fontSize: 11,
                  color: Colors.white38,
                ),
              ),
          ],

          const Spacer(),

          // Repo path selector
          _RepoPicker(repoPath: repoPath),
          const SizedBox(width: 12),

          // Activity toggle
          IconButton(
            icon: Icon(
              showActivity
                  ? Icons.history_toggle_off
                  : Icons.history,
              size: 18,
            ),
            tooltip: showActivity ? 'Hide activity' : 'Show activity',
            color: showActivity ? ClawdTheme.clawLight : Colors.white38,
            onPressed: onToggleActivity,
            padding: const EdgeInsets.all(6),
            constraints: const BoxConstraints(),
          ),
          const SizedBox(width: 4),

          // Refresh button
          IconButton(
            icon: const Icon(Icons.refresh, size: 18),
            tooltip: 'Refresh',
            color: Colors.white54,
            onPressed: () {
              ref.invalidate(taskListProvider);
              ref.invalidate(activityFeedProvider);
              ref.invalidate(agentListProvider);
              ref.invalidate(taskSummaryProvider);
            },
            padding: const EdgeInsets.all(6),
            constraints: const BoxConstraints(),
          ),
        ],
      ),
    );
  }
}

// ── Repo picker ───────────────────────────────────────────────────────────────

class _RepoPicker extends ConsumerWidget {
  const _RepoPicker({required this.repoPath});

  final String? repoPath;

  String _label(String? path) {
    if (path == null) return 'All Projects';
    final parts = path.replaceAll(r'\', '/').split('/');
    return parts.where((p) => p.isNotEmpty).lastOrNull ?? path;
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Repo list is not available from DaemonInfo — user selects via text input.
    // Show only "All Projects" plus the currently active repo if set.
    final allRepos = repoPath != null ? [repoPath!] : <String>[];

    return PopupMenuButton<String?>(
      initialValue: repoPath,
      tooltip: 'Select project',
      color: ClawdTheme.surfaceElevated,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.folder_outlined, size: 14, color: Colors.white54),
          const SizedBox(width: 6),
          Text(
            _label(repoPath),
            style: const TextStyle(
              fontSize: 12,
              color: Colors.white70,
            ),
          ),
          const SizedBox(width: 4),
          const Icon(Icons.arrow_drop_down, size: 16, color: Colors.white38),
        ],
      ),
      onSelected: (path) {
        ref.read(activeRepoPathProvider.notifier).state = path;
        ref.read(selectedTaskIdProvider.notifier).state = null;
      },
      itemBuilder: (_) => [
        const PopupMenuItem<String?>(
          value: null,
          child: Text('All Projects'),
        ),
        ...allRepos.map(
          (path) => PopupMenuItem<String?>(
            value: path,
            child: Text(_label(path)),
          ),
        ),
      ],
    );
  }
}

// ── Filter bar ────────────────────────────────────────────────────────────────

class _FilterBar extends StatelessWidget {
  const _FilterBar({required this.filter, required this.onClear});

  final DashboardFilter filter;
  final VoidCallback onClear;

  @override
  Widget build(BuildContext context) {
    final chips = <String>[];
    if (filter.agent != null) chips.add('agent: ${filter.agent}');
    if (filter.severity != null) chips.add('severity: ${filter.severity}');
    if (filter.taskType != null) chips.add('type: ${filter.taskType}');
    if (filter.phase != null) chips.add('phase: ${filter.phase}');

    return Container(
      padding: const EdgeInsets.fromLTRB(16, 6, 16, 6),
      color: ClawdTheme.surfaceElevated.withValues(alpha: 0.7),
      child: Row(
        children: [
          const Icon(Icons.filter_list, size: 14, color: Colors.white38),
          const SizedBox(width: 8),
          Expanded(
            child: Wrap(
              spacing: 6,
              children: chips
                  .map(
                    (c) => Chip(
                      label: Text(
                        c,
                        style: const TextStyle(
                          fontSize: 11,
                          color: Colors.white70,
                        ),
                      ),
                      backgroundColor: ClawdTheme.claw.withValues(alpha: 0.15),
                      side: const BorderSide(color: ClawdTheme.surfaceBorder),
                      padding: EdgeInsets.zero,
                      visualDensity: VisualDensity.compact,
                    ),
                  )
                  .toList(),
            ),
          ),
          TextButton(
            onPressed: onClear,
            style: TextButton.styleFrom(
              foregroundColor: Colors.white38,
              padding: const EdgeInsets.symmetric(horizontal: 8),
            ),
            child: const Text('Clear', style: TextStyle(fontSize: 11)),
          ),
        ],
      ),
    );
  }
}
