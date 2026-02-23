/// Session types for the clawd daemon.

import 'dart:developer' as dev;

enum SessionStatus { idle, running, paused, completed, error }

enum ProviderType { claude, codex, cursor }

class Session {
  final String id;
  final String repoPath;
  final String title;
  final ProviderType provider;
  final SessionStatus status;
  final DateTime createdAt;
  final DateTime updatedAt;
  final int messageCount;

  const Session({
    required this.id,
    required this.repoPath,
    required this.title,
    required this.provider,
    required this.status,
    required this.createdAt,
    required this.updatedAt,
    required this.messageCount,
  });

  factory Session.fromJson(Map<String, dynamic> json) {
    final providerStr = json['provider'] as String? ?? '';
    final statusStr = json['status'] as String? ?? 'idle';
    return Session(
      id: json['id'] as String,
      repoPath: json['repoPath'] as String,
      title: json['title'] as String? ?? '',
      provider: _parseProvider(providerStr),
      status: _parseStatus(statusStr),
      createdAt: DateTime.parse(json['createdAt'] as String),
      updatedAt: DateTime.parse(json['updatedAt'] as String),
      messageCount: json['messageCount'] as int? ?? 0,
    );
  }

  static ProviderType _parseProvider(String s) {
    try {
      return ProviderType.values.byName(s);
    } catch (_) {
      dev.log('unknown provider: $s', name: 'clawd_proto');
      return ProviderType.claude;
    }
  }

  static SessionStatus _parseStatus(String s) {
    try {
      return SessionStatus.values.byName(s);
    } catch (_) {
      dev.log('unknown status: $s', name: 'clawd_proto');
      return SessionStatus.idle;
    }
  }
}
