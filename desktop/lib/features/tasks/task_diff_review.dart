import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

// Provider that fetches the unified diff for a worktree via worktrees.list.
final _taskDiffProvider = FutureProvider.family<String, String>((ref, taskId) async {
  final client = ref.read(daemonProvider.notifier).client;
  try {
    final result = await client.call<Map<String, dynamic>>(
      'worktrees.diff',
      {'task_id': taskId},
    );
    return result['diff'] as String? ?? '';
  } catch (_) {
    return '';
  }
});

/// Opens a task's worktree diff for review.
/// Shows a unified diff in a scrollable monospace text view.
/// [Accept All] and [Reject All] are stubs — per-hunk review is future work.
class TaskDiffReview extends ConsumerWidget {
  const TaskDiffReview({required this.taskId, super.key});

  final String taskId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final diffAsync = ref.watch(_taskDiffProvider(taskId));
    final summaryAsync = ref.watch(taskSummaryProvider(taskId));

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
          child: Row(
            children: [
              const Icon(Icons.difference_outlined, size: 16,
                  color: ClawdTheme.clawLight),
              const SizedBox(width: 8),
              const Text(
                'Diff Review',
                style: TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  color: Colors.white,
                ),
              ),
              const SizedBox(width: 8),
              Text(
                taskId,
                style: const TextStyle(
                  fontSize: 11,
                  fontFamily: 'monospace',
                  color: Colors.white38,
                ),
              ),
              const Spacer(),
              // Action buttons
              _ActionButton(
                label: 'Accept All',
                color: Colors.green,
                icon: Icons.check_circle_outline,
                onTap: () => _showStubDialog(context, 'Accept All'),
              ),
              const SizedBox(width: 8),
              _ActionButton(
                label: 'Reject All',
                color: Colors.red,
                icon: Icons.cancel_outlined,
                onTap: () => _showStubDialog(context, 'Reject All'),
              ),
            ],
          ),
        ),

        // ── Summary stats ────────────────────────────────────────────────────
        summaryAsync.when(
          data: (summary) => _SummaryBar(summary: summary),
          loading: () => const SizedBox.shrink(),
          error: (_, __) => const SizedBox.shrink(),
        ),

        // ── Diff content ─────────────────────────────────────────────────────
        Expanded(
          child: diffAsync.when(
            loading: () => const Center(
              child: CircularProgressIndicator(color: ClawdTheme.claw),
            ),
            error: (e, _) => ErrorState(
              icon: Icons.error_outline,
              title: 'Failed to load diff',
              description: e.toString(),
              onRetry: () => ref.refresh(_taskDiffProvider(taskId)),
            ),
            data: (diff) {
              if (diff.isEmpty) {
                return const EmptyState(
                  icon: Icons.difference_outlined,
                  title: 'No diff available',
                  subtitle: 'The worktree may not have any uncommitted changes.',
                );
              }
              return _DiffView(diff: diff);
            },
          ),
        ),
      ],
    );
  }

  void _showStubDialog(BuildContext context, String action) {
    showDialog<void>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: ClawdTheme.surfaceElevated,
        title: Text(
          action,
          style: const TextStyle(color: Colors.white, fontSize: 15),
        ),
        content: const Text(
          'Per-hunk diff review is not yet wired to the daemon. '
          'This stub will be replaced when worktrees.accept/reject RPCs are added.',
          style: TextStyle(color: Colors.white70, fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('OK'),
          ),
        ],
      ),
    );
  }
}

// ── Summary bar ────────────────────────────────────────────────────────────────

class _SummaryBar extends StatelessWidget {
  const _SummaryBar({required this.summary});
  final TaskChangeSummary summary;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      decoration: const BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        border: Border(bottom: BorderSide(color: ClawdTheme.surfaceBorder)),
      ),
      child: Row(
        children: [
          _Stat(label: 'Files', value: '${summary.filesChanged}'),
          const SizedBox(width: 16),
          _Stat(
            label: '+',
            value: '${summary.linesAdded}',
            color: Colors.green,
          ),
          const SizedBox(width: 16),
          _Stat(
            label: '−',
            value: '${summary.linesRemoved}',
            color: Colors.red,
          ),
          if (summary.testsRun > 0) ...[
            const SizedBox(width: 16),
            _Stat(
              label: 'Tests',
              value: '${summary.testsPassed}/${summary.testsRun}',
              color: summary.testsPassed == summary.testsRun
                  ? Colors.green
                  : Colors.red,
            ),
          ],
          if (summary.riskFlags.isNotEmpty) ...[
            const SizedBox(width: 16),
            const Icon(Icons.warning_amber, size: 12, color: Colors.amber),
            const SizedBox(width: 4),
            Text(
              summary.riskFlags.join(', '),
              style: const TextStyle(fontSize: 11, color: Colors.amber),
            ),
          ],
        ],
      ),
    );
  }
}

class _Stat extends StatelessWidget {
  const _Stat({required this.label, required this.value, this.color});
  final String label;
  final String value;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          '$label ',
          style: const TextStyle(fontSize: 11, color: Colors.white38),
        ),
        Text(
          value,
          style: TextStyle(
            fontSize: 11,
            fontWeight: FontWeight.w600,
            color: color ?? Colors.white70,
          ),
        ),
      ],
    );
  }
}

// ── Diff view ──────────────────────────────────────────────────────────────────

class _DiffView extends StatelessWidget {
  const _DiffView({required this.diff});
  final String diff;

  @override
  Widget build(BuildContext context) {
    final lines = diff.split('\n');
    return ListView.builder(
      padding: const EdgeInsets.symmetric(vertical: 4),
      itemCount: lines.length,
      itemBuilder: (context, i) => _DiffLine(line: lines[i]),
    );
  }
}

class _DiffLine extends StatelessWidget {
  const _DiffLine({required this.line});
  final String line;

  Color get _bg {
    if (line.startsWith('+') && !line.startsWith('+++')) {
      return Colors.green.withValues(alpha: 0.07);
    }
    if (line.startsWith('-') && !line.startsWith('---')) {
      return Colors.red.withValues(alpha: 0.07);
    }
    if (line.startsWith('@@')) {
      return ClawdTheme.claw.withValues(alpha: 0.08);
    }
    return Colors.transparent;
  }

  Color get _textColor {
    if (line.startsWith('+') && !line.startsWith('+++')) return Colors.green;
    if (line.startsWith('-') && !line.startsWith('---')) return Colors.red;
    if (line.startsWith('@@')) return ClawdTheme.clawLight;
    if (line.startsWith('diff ') || line.startsWith('index ') ||
        line.startsWith('---') || line.startsWith('+++')) {
      return Colors.white54;
    }
    return Colors.white70;
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      color: _bg,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 1),
      child: Text(
        line,
        style: TextStyle(
          fontSize: 11,
          fontFamily: 'monospace',
          color: _textColor,
          height: 1.6,
        ),
      ),
    );
  }
}

// ── Action button ──────────────────────────────────────────────────────────────

class _ActionButton extends StatelessWidget {
  const _ActionButton({
    required this.label,
    required this.color,
    required this.icon,
    required this.onTap,
  });
  final String label;
  final Color color;
  final IconData icon;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(6),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
        decoration: BoxDecoration(
          color: color.withValues(alpha: 0.12),
          borderRadius: BorderRadius.circular(6),
          border: Border.all(color: color.withValues(alpha: 0.3)),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 13, color: color),
            const SizedBox(width: 5),
            Text(
              label,
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w600,
                color: color,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
