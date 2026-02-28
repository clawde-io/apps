import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';

/// A list tile for a single agent task.
///
/// Shows task title, status chip, and — when a pending worktree diff exists —
/// a small amber badge so the user knows the task is waiting for review.
class TaskListTile extends ConsumerWidget {
  const TaskListTile({
    required this.task,
    this.onTap,
    this.onDiffTap,
    super.key,
  });

  final AgentTask task;

  /// Called when the main tile body is tapped (navigate to task detail).
  final VoidCallback? onTap;

  /// Called when the diff badge is tapped (open the diff review screen).
  final VoidCallback? onDiffTap;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return ListTile(
      onTap: onTap,
      title: Text(
        task.title,
        maxLines: 2,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: _StatusChip(status: task.status),
      trailing: _DiffBadge(taskId: task.id, onTap: onDiffTap),
    );
  }
}

// ─── Status chip ──────────────────────────────────────────────────────────────

class _StatusChip extends StatelessWidget {
  const _StatusChip({required this.status});

  final TaskStatus status;

  static const _colorMap = {
    TaskStatus.pending: Colors.grey,
    TaskStatus.planned: Colors.grey,
    TaskStatus.active: Colors.blue,
    TaskStatus.inProgress: Colors.blue,
    TaskStatus.codeReview: Colors.orange,
    TaskStatus.inQa: Colors.purple,
    TaskStatus.done: Colors.green,
    TaskStatus.blocked: Colors.red,
    TaskStatus.deferred: Colors.brown,
    TaskStatus.canceled: Colors.brown,
    TaskStatus.failed: Colors.red,
  };

  @override
  Widget build(BuildContext context) {
    final color = _colorMap[status] ?? Colors.grey;
    final label = status.toJsonStr().replaceAll('_', ' ');
    return Padding(
      padding: const EdgeInsets.only(top: 4),
      child: Text(
        label,
        style: TextStyle(fontSize: 11, color: color, fontWeight: FontWeight.w600),
      ),
    );
  }
}

// ─── Diff badge ───────────────────────────────────────────────────────────────

/// Amber badge shown when a worktree for this task is in a reviewable state.
///
/// [WorktreeInfo.status] is a plain String from the daemon JSON response.
/// Reviewable states: `"active"` (changes made, not yet merged).
class _DiffBadge extends ConsumerWidget {
  const _DiffBadge({required this.taskId, this.onTap});

  final String taskId;
  final VoidCallback? onTap;

  static const _reviewableStatuses = {'active', 'needs_review'};

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final asyncWorktree = ref.watch(worktreeProvider(taskId));

    return asyncWorktree.when(
      loading: () => const SizedBox.shrink(),
      error: (_, __) => const SizedBox.shrink(),
      data: (worktree) {
        if (worktree == null) return const SizedBox.shrink();
        if (!_reviewableStatuses.contains(worktree.status)) {
          return const SizedBox.shrink();
        }

        return GestureDetector(
          onTap: onTap,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
            decoration: BoxDecoration(
              color: Colors.amber.shade700,
              borderRadius: BorderRadius.circular(12),
            ),
            child: const Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(Icons.difference_outlined, size: 13, color: Colors.white),
                SizedBox(width: 3),
                Text(
                  'Review',
                  style: TextStyle(
                    fontSize: 11,
                    color: Colors.white,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}
