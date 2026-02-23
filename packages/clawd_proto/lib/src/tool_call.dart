/// Tool call and result types for clawd sessions.
library;

import 'dart:convert';
import 'dart:developer' as dev;

/// Daemon sends `"done"` for finished tool calls; `completed` is the Dart
/// alias. Both are valid — `done` is included for exhaustive switch correctness.
enum ToolCallStatus { pending, running, completed, done, error }

class ToolCall {
  final String id;
  final String sessionId;
  final String? messageId;
  final String toolName;
  final Map<String, dynamic> input;
  final ToolCallStatus status;
  final DateTime createdAt;
  final DateTime? completedAt;

  const ToolCall({
    required this.id,
    required this.sessionId,
    this.messageId,
    required this.toolName,
    required this.input,
    required this.status,
    required this.createdAt,
    this.completedAt,
  });

  /// Parse from a push-event toolCall sub-object. The [sessionId] is pulled
  /// from the outer params object and passed in separately.
  factory ToolCall.fromJson(Map<String, dynamic> json) => ToolCall(
        id: json['id'] as String,
        sessionId: json['sessionId'] as String? ?? '',
        messageId: json['messageId'] as String?,
        toolName: json['name'] as String? ?? json['toolName'] as String? ?? '',
        input: _parseInput(json['input']),
        status: _parseStatus(json['status'] as String? ?? 'running'),
        createdAt: DateTime.parse(json['createdAt'] as String),
        completedAt: json['completedAt'] != null
            ? DateTime.parse(json['completedAt'] as String)
            : null,
      );

  static ToolCallStatus _parseStatus(String s) {
    try {
      return ToolCallStatus.values.byName(s);
    } catch (_) {
      return ToolCallStatus.error;
    }
  }

  static Map<String, dynamic> _parseInput(dynamic raw) {
    if (raw is Map<String, dynamic>) return raw;
    // Daemon may store input as a JSON string (from serde_json::to_string).
    if (raw is String && raw.isNotEmpty) {
      try {
        final decoded = jsonDecode(raw);
        if (decoded is Map<String, dynamic>) return decoded;
      } catch (_) {
        dev.log('Failed to parse tool input: $raw', name: 'clawd_proto');
      }
    }
    return const {};
  }
}

class ToolResult {
  final String toolCallId;
  final bool isError;
  final dynamic output;

  const ToolResult({
    required this.toolCallId,
    required this.isError,
    this.output,
  });

  factory ToolResult.fromJson(Map<String, dynamic> json) => ToolResult(
        toolCallId: json['toolCallId'] as String,
        isError: json['isError'] as bool,
        output: json['output'],
      );
}
