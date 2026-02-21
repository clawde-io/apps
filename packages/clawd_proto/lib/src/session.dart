/// Session types for the clawd daemon.
library;

enum SessionStatus { idle, running, paused, completed, error }

enum ProviderType { claude, codex, cursor }

class Session {
  final String id;
  final String repoPath;
  final ProviderType provider;
  final SessionStatus status;
  final DateTime createdAt;
  final DateTime? startedAt;
  final DateTime? endedAt;
  final Map<String, dynamic> metadata;

  const Session({
    required this.id,
    required this.repoPath,
    required this.provider,
    required this.status,
    required this.createdAt,
    this.startedAt,
    this.endedAt,
    this.metadata = const {},
  });

  factory Session.fromJson(Map<String, dynamic> json) => Session(
        id: json['id'] as String,
        repoPath: json['repo_path'] as String,
        provider: ProviderType.values.byName(json['provider'] as String),
        status: SessionStatus.values.byName(json['status'] as String),
        createdAt: DateTime.parse(json['created_at'] as String),
        startedAt: json['started_at'] != null
            ? DateTime.parse(json['started_at'] as String)
            : null,
        endedAt: json['ended_at'] != null
            ? DateTime.parse(json['ended_at'] as String)
            : null,
        metadata:
            (json['metadata'] as Map<String, dynamic>?) ?? const {},
      );
}
