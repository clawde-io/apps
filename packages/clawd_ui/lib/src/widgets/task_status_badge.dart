import 'package:flutter/material.dart';
import 'package:clawd_proto/clawd_proto.dart';

/// Colored pill badge showing a task status.
class TaskStatusBadge extends StatelessWidget {
  const TaskStatusBadge({super.key, required this.status, this.compact = false});

  final TaskStatus status;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final (color, label) = _meta(status);
    if (compact) {
      return Container(
        width: 8,
        height: 8,
        decoration: BoxDecoration(color: color, shape: BoxShape.circle),
      );
    }
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Text(
        label,
        style: TextStyle(fontSize: 11, fontWeight: FontWeight.w600, color: color),
      ),
    );
  }

  static (Color, String) _meta(TaskStatus s) => switch (s) {
        TaskStatus.pending => (const Color(0xFF9E9E9E), 'Pending'),
        TaskStatus.inProgress => (const Color(0xFF42A5F5), 'In Progress'),
        TaskStatus.done => (const Color(0xFF66BB6A), 'Done'),
        TaskStatus.blocked => (const Color(0xFFEF5350), 'Blocked'),
        TaskStatus.deferred => (const Color(0xFFFFCA28), 'Deferred'),
        TaskStatus.interrupted => (const Color(0xFFFF7043), 'Interrupted'),
        TaskStatus.inQa => (const Color(0xFFAB47BC), 'In QA'),
      };

  static Color colorFor(TaskStatus s) => _meta(s).$1;
}
