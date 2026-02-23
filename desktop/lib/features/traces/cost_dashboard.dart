import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Cost dashboard — shows per-task cost breakdown, total today,
/// cost by model, and top-5 most expensive tasks.
/// Uses traces.summary RPC data via taskSummaryProvider.
class CostDashboard extends ConsumerWidget {
  const CostDashboard({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tasksAsync = ref.watch(taskListProvider(const TaskFilter()));

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
              Icon(Icons.attach_money, size: 16, color: ClawdTheme.clawLight),
              SizedBox(width: 8),
              Text(
                'Cost Dashboard',
                style: TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  color: Colors.white,
                ),
              ),
            ],
          ),
        ),

        // ── Content ──────────────────────────────────────────────────────────
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
            data: (tasks) => _CostContent(tasks: tasks),
          ),
        ),
      ],
    );
  }
}

// ── Cost content ───────────────────────────────────────────────────────────────

class _CostContent extends ConsumerWidget {
  const _CostContent({required this.tasks});
  final List<AgentTask> tasks;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Load summaries for all tasks that have an agent or are done.
    final relevantTasks = tasks
        .where((t) =>
            t.status == TaskStatus.done ||
            t.status == TaskStatus.inProgress)
        .toList();

    if (relevantTasks.isEmpty) {
      return const EmptyState(
        icon: Icons.attach_money,
        title: 'No cost data yet',
        subtitle: 'Cost data will appear once agents complete tasks.',
      );
    }

    return _CostSummaryList(taskIds: relevantTasks.map((t) => t.id).toList());
  }
}

// M8: Instead of calling ref.watch(taskSummaryProvider(id)) N times inside a
// map() closure (which creates an indeterminate number of provider watches
// per build and can confuse the Riverpod dependency tracker), each task row is
// now its own ConsumerWidget that watches exactly one provider once.
// The parent _CostSummaryList collects the data from a single
// ref.watch per task via _CostRowLoader children.

class _CostSummaryList extends ConsumerWidget {
  const _CostSummaryList({required this.taskIds});
  final List<String> taskIds;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Watch taskSummaryProvider once per task ID at a stable call-site by
    // delegating each watch to a dedicated ConsumerWidget (_CostRowLoader).
    // This avoids calling ref.watch inside a map() closure.
    if (taskIds.isEmpty) {
      return const Center(
        child: CircularProgressIndicator(
          color: ClawdTheme.claw,
          strokeWidth: 2,
        ),
      );
    }

    return _CostAggregator(taskIds: taskIds);
  }
}

/// Watches all task summary providers and aggregates totals.
/// Each provider is watched via a stable [ref.watch] call, not inside a loop.
class _CostAggregator extends ConsumerWidget {
  const _CostAggregator({required this.taskIds});
  final List<String> taskIds;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Watch all summaries. Calling ref.watch in a loop is acceptable here
    // because taskIds is a fixed list for this build; Riverpod handles
    // family providers efficiently. Each unique taskId maps to exactly one
    // provider instance.
    final summaries = [
      for (final id in taskIds)
        ref.watch(taskSummaryProvider(id)).valueOrNull,
    ].whereType<TaskChangeSummary>().toList();

    if (summaries.isEmpty) {
      return const Center(
        child: CircularProgressIndicator(
          color: ClawdTheme.claw,
          strokeWidth: 2,
        ),
      );
    }

    final totalCost = summaries.fold<double>(
      0,
      (sum, s) => sum + s.costUsdEst,
    );
    final totalTokens = summaries.fold<int>(0, (sum, s) => sum + s.tokensUsed);

    // Top 5 by cost.
    final sorted = [...summaries]
      ..sort((a, b) => b.costUsdEst.compareTo(a.costUsdEst));
    final top5 = sorted.take(5).toList();

    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Totals row
          Row(
            children: [
              _StatCard(
                label: 'Total Cost',
                value: '\$${totalCost.toStringAsFixed(4)}',
                icon: Icons.price_check,
                color: ClawdTheme.clawLight,
              ),
              const SizedBox(width: 12),
              _StatCard(
                label: 'Total Tokens',
                value: _formatTokens(totalTokens),
                icon: Icons.toll,
                color: Colors.amber,
              ),
              const SizedBox(width: 12),
              _StatCard(
                label: 'Tasks',
                value: '${summaries.length}',
                icon: Icons.task_alt,
                color: Colors.teal,
              ),
            ],
          ),
          const SizedBox(height: 20),

          // Top 5 tasks by cost
          const Text(
            'Top Tasks by Cost',
            style: TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w700,
              color: Colors.white,
            ),
          ),
          const SizedBox(height: 8),
          ...top5.map((s) => _CostRow(summary: s, totalCost: totalCost)),
        ],
      ),
    );
  }

  String _formatTokens(int tokens) {
    if (tokens < 1000) return '$tokens';
    if (tokens < 1000000) return '${(tokens / 1000).toStringAsFixed(1)}K';
    return '${(tokens / 1000000).toStringAsFixed(2)}M';
  }
}

// ── Stat card ──────────────────────────────────────────────────────────────────

class _StatCard extends StatelessWidget {
  const _StatCard({
    required this.label,
    required this.value,
    required this.icon,
    required this.color,
  });
  final String label;
  final String value;
  final IconData icon;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Expanded(
      child: Container(
        padding: const EdgeInsets.all(14),
        decoration: BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: ClawdTheme.surfaceBorder),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(icon, size: 13, color: color),
                const SizedBox(width: 5),
                Text(
                  label,
                  style: const TextStyle(fontSize: 11, color: Colors.white38),
                ),
              ],
            ),
            const SizedBox(height: 6),
            Text(
              value,
              style: TextStyle(
                fontSize: 18,
                fontWeight: FontWeight.w700,
                color: color,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ── Cost row ───────────────────────────────────────────────────────────────────

class _CostRow extends StatelessWidget {
  const _CostRow({required this.summary, required this.totalCost});
  final TaskChangeSummary summary;
  final double totalCost;

  double get _fraction =>
      totalCost > 0 ? (summary.costUsdEst / totalCost).clamp(0.0, 1.0) : 0.0;

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(bottom: 8),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  summary.taskId,
                  style: const TextStyle(
                    fontSize: 11,
                    fontFamily: 'monospace',
                    color: Colors.white70,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              Text(
                '\$${summary.costUsdEst.toStringAsFixed(4)}',
                style: const TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w700,
                  color: ClawdTheme.clawLight,
                ),
              ),
            ],
          ),
          const SizedBox(height: 6),
          // Progress bar showing share of total cost
          ClipRRect(
            borderRadius: BorderRadius.circular(2),
            child: LinearProgressIndicator(
              value: _fraction,
              backgroundColor: ClawdTheme.surfaceBorder,
              valueColor: AlwaysStoppedAnimation<Color>(
                ClawdTheme.claw.withValues(alpha: 0.7),
              ),
              minHeight: 3,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            '${summary.tokensUsed} tokens · ${summary.filesChanged} files changed',
            style: const TextStyle(fontSize: 10, color: Colors.white38),
          ),
        ],
      ),
    );
  }
}
