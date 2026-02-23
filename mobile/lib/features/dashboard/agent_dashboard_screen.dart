import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Mobile agent dashboard — tabbed: Board | Activity | Agents.
/// Task detail opens as a bottom sheet.
class AgentDashboardScreen extends ConsumerStatefulWidget {
  const AgentDashboardScreen({super.key});

  @override
  ConsumerState<AgentDashboardScreen> createState() =>
      _AgentDashboardScreenState();
}

class _AgentDashboardScreenState
    extends ConsumerState<AgentDashboardScreen>
    with SingleTickerProviderStateMixin {
  late final TabController _tabs;

  @override
  void initState() {
    super.initState();
    _tabs = TabController(length: 3, vsync: this);
  }

  @override
  void dispose() {
    _tabs.dispose();
    super.dispose();
  }

  void _openTaskDetail(BuildContext context, AgentTask task) {
    ref.read(selectedTaskIdProvider.notifier).state = task.id;
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      backgroundColor: Colors.transparent,
      builder: (_) => _TaskDetailSheet(task: task),
    );
  }

  @override
  Widget build(BuildContext context) {
    final repoPath = ref.watch(activeRepoPathProvider);
    final byStatus = ref.watch(tasksByStatusProvider(repoPath));
    final summaryAsync = ref.watch(taskSummaryProvider(repoPath));
    final activityAsync = ref.watch(activityFeedProvider(repoPath));
    final agentsAsync = ref.watch(agentListProvider(repoPath));

    final total = summaryAsync.valueOrNull?.total ?? 0;
    final inProgress = summaryAsync.valueOrNull?.byStatus['in_progress'] ?? 0;

    return Scaffold(
      appBar: AppBar(
        title: Row(
          children: [
            const Text('Tasks'),
            if (total > 0) ...[
              const SizedBox(width: 8),
              _CountBadge(count: total, color: ClawdTheme.claw),
            ],
            if (inProgress > 0) ...[
              const SizedBox(width: 6),
              _CountBadge(
                count: inProgress,
                color: Colors.green,
                label: 'active',
              ),
            ],
          ],
        ),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            tooltip: 'Refresh',
            onPressed: () {
              ref.invalidate(taskListProvider);
              ref.invalidate(activityFeedProvider);
              ref.invalidate(agentListProvider);
              ref.invalidate(taskSummaryProvider);
            },
          ),
        ],
        bottom: TabBar(
          controller: _tabs,
          tabs: const [
            Tab(text: 'Board'),
            Tab(text: 'Activity'),
            Tab(text: 'Agents'),
          ],
        ),
      ),
      body: TabBarView(
        controller: _tabs,
        children: [
          // ── Board tab ──────────────────────────────────────────────────
          KanbanBoard(
            tasksByStatus: byStatus,
            onTaskTap: (task) => _openTaskDetail(context, task),
          ),

          // ── Activity tab ───────────────────────────────────────────────
          activityAsync.when(
            loading: () =>
                const Center(child: CircularProgressIndicator()),
            error: (e, _) => Center(
              child: Text(
                'Failed to load activity\n$e',
                textAlign: TextAlign.center,
                style: const TextStyle(color: Colors.white54),
              ),
            ),
            data: (entries) => entries.isEmpty
                ? const Center(
                    child: Text(
                      'No activity yet',
                      style: TextStyle(color: Colors.white54),
                    ),
                  )
                : ActivityFeed(entries: entries),
          ),

          // ── Agents tab ─────────────────────────────────────────────────
          agentsAsync.when(
            loading: () =>
                const Center(child: CircularProgressIndicator()),
            error: (e, _) => Center(
              child: Text('Failed to load agents\n$e',
                  textAlign: TextAlign.center,
                  style: const TextStyle(color: Colors.white54)),
            ),
            data: (agents) => agents.isEmpty
                ? const Center(
                    child: Text(
                      'No agents connected',
                      style: TextStyle(color: Colors.white54),
                    ),
                  )
                : ListView.separated(
                    padding: const EdgeInsets.all(16),
                    itemCount: agents.length,
                    separatorBuilder: (_, __) =>
                        const SizedBox(height: 8),
                    itemBuilder: (_, i) =>
                        _AgentTile(agent: agents[i]),
                  ),
          ),
        ],
      ),
    );
  }
}

// ── Count badge ───────────────────────────────────────────────────────────────

class _CountBadge extends StatelessWidget {
  const _CountBadge({required this.count, required this.color, this.label});

  final int count;
  final Color color;
  final String? label;

  @override
  Widget build(BuildContext context) {
    final text = label != null ? '$count $label' : '$count';
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.2),
        borderRadius: BorderRadius.circular(10),
      ),
      child: Text(
        text,
        style: TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w600,
          color: color,
        ),
      ),
    );
  }
}

// ── Agent tile ────────────────────────────────────────────────────────────────

class _AgentTile extends StatelessWidget {
  const _AgentTile({required this.agent});
  final AgentView agent;

  @override
  Widget build(BuildContext context) {
    final isActive = agent.status == AgentStatus.active;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Row(
        children: [
          Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(
              color: isActive ? Colors.green : Colors.white24,
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  agent.agentId,
                  style: const TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    color: Colors.white,
                  ),
                ),
                if (agent.agentType.isNotEmpty)
                  Text(
                    agent.agentType,
                    style: const TextStyle(
                      fontSize: 11,
                      color: Colors.white54,
                    ),
                  ),
              ],
            ),
          ),
          Text(
            isActive ? 'Active' : 'Idle',
            style: TextStyle(
              fontSize: 11,
              color: isActive ? Colors.green : Colors.white38,
            ),
          ),
        ],
      ),
    );
  }
}

// ── Task detail bottom sheet ──────────────────────────────────────────────────

class _TaskDetailSheet extends ConsumerWidget {
  const _TaskDetailSheet({required this.task});
  final AgentTask task;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return DraggableScrollableSheet(
      initialChildSize: 0.65,
      minChildSize: 0.4,
      maxChildSize: 0.95,
      snap: true,
      snapSizes: const [0.65, 0.95],
      builder: (_, scrollController) => Container(
        decoration: const BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
        ),
        child: Column(
          children: [
            // Drag handle
            Container(
              margin: const EdgeInsets.only(top: 8),
              width: 36,
              height: 4,
              decoration: BoxDecoration(
                color: Colors.white24,
                borderRadius: BorderRadius.circular(2),
              ),
            ),
            Expanded(
              child: TaskDetailPanel(
                task: task,
                onClose: () => Navigator.pop(context),
                onMarkDone: (notes) async {
                  final client =
                      ref.read(daemonProvider.notifier).client;
                  await client.updateTaskStatus(
                    task.id,
                    'done',
                    notes: notes.isEmpty ? null : notes,
                  );
                  if (context.mounted) Navigator.pop(context);
                },
                onMarkBlocked: () async {
                  final client =
                      ref.read(daemonProvider.notifier).client;
                  await client.updateTaskStatus(task.id, 'blocked');
                  if (context.mounted) Navigator.pop(context);
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}
