/// Message types for session chat history.
library;

enum MessageRole { user, assistant, system, tool }

class Message {
  final String id;
  final String sessionId;
  final MessageRole role;
  final String content;
  final String status;
  final DateTime createdAt;
  final Map<String, dynamic> metadata;

  const Message({
    required this.id,
    required this.sessionId,
    required this.role,
    required this.content,
    required this.status,
    required this.createdAt,
    this.metadata = const {},
  });

  factory Message.fromJson(Map<String, dynamic> json) => Message(
        id: json['id'] as String,
        sessionId: json['sessionId'] as String,
        role: MessageRole.values.byName(json['role'] as String),
        content: json['content'] as String,
        status: json['status'] as String? ?? 'done',
        createdAt: DateTime.parse(json['createdAt'] as String),
        metadata:
            (json['metadata'] as Map<String, dynamic>?) ?? const {},
      );
}
