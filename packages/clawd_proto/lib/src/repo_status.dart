/// Repository status types for git integration.
library;

class RepoStatus {
  final String path;
  final String? branch;
  final bool isDirty;
  final int aheadBy;
  final int behindBy;
  final List<FileStatus> files;

  const RepoStatus({
    required this.path,
    this.branch,
    required this.isDirty,
    required this.aheadBy,
    required this.behindBy,
    required this.files,
  });

  factory RepoStatus.fromJson(Map<String, dynamic> json) => RepoStatus(
        path: json['path'] as String,
        branch: json['branch'] as String?,
        isDirty: json['is_dirty'] as bool,
        aheadBy: json['ahead_by'] as int,
        behindBy: json['behind_by'] as int,
        files: (json['files'] as List<dynamic>)
            .map((f) => FileStatus.fromJson(f as Map<String, dynamic>))
            .toList(),
      );
}

enum FileState { untracked, modified, staged, deleted, renamed, conflicted }

class FileStatus {
  final String path;
  final FileState state;
  final String? oldPath; // for renames

  const FileStatus({
    required this.path,
    required this.state,
    this.oldPath,
  });

  factory FileStatus.fromJson(Map<String, dynamic> json) => FileStatus(
        path: json['path'] as String,
        state: FileState.values.byName(json['state'] as String),
        oldPath: json['old_path'] as String?,
      );
}
