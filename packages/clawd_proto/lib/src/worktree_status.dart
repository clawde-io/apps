/// Worktree status returned by the daemon for a given task.
class WorktreeStatus {
  final String taskId;
  final String branch;
  final String path;
  final int changeCount;
  final bool pendingMerge;
  final bool isStale;

  const WorktreeStatus({
    required this.taskId,
    required this.branch,
    required this.path,
    required this.changeCount,
    required this.pendingMerge,
    required this.isStale,
  });

  factory WorktreeStatus.fromJson(Map<String, dynamic> json) => WorktreeStatus(
        taskId: json['task_id'] as String? ?? json['taskId'] as String,
        branch: json['branch'] as String,
        path: json['path'] as String,
        changeCount: (json['change_count'] as num?)?.toInt() ??
            (json['changeCount'] as num?)?.toInt() ??
            0,
        pendingMerge: json['pending_merge'] as bool? ??
            json['pendingMerge'] as bool? ??
            false,
        isStale: json['is_stale'] as bool? ?? json['isStale'] as bool? ?? false,
      );

  Map<String, dynamic> toJson() => {
        'task_id': taskId,
        'branch': branch,
        'path': path,
        'change_count': changeCount,
        'pending_merge': pendingMerge,
        'is_stale': isStale,
      };
}
