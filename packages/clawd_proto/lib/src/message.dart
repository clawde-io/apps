/// Message types for session chat history.
library;

enum MessageRole { user, assistant, system, tool }

class Message {
  final String id;
  final String sessionId;
  final MessageRole role;
  final String content;
  final DateTime timestamp;
  final Map<String, dynamic> metadata;

  const Message({
    required this.id,
    required this.sessionId,
    required this.role,
    required this.content,
    required this.timestamp,
    this.metadata = const {},
  });

  factory Message.fromJson(Map<String, dynamic> json) => Message(
        id: json['id'] as String,
        sessionId: json['session_id'] as String,
        role: MessageRole.values.byName(json['role'] as String),
        content: json['content'] as String,
        timestamp: DateTime.parse(json['timestamp'] as String),
        metadata:
            (json['metadata'] as Map<String, dynamic>?) ?? const {},
      );
}
