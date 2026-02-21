/// Tool call and result types for clawd sessions.
library;

enum ToolCallStatus { pending, running, completed, error }

class ToolCall {
  final String id;
  final String sessionId;
  final String toolName;
  final Map<String, dynamic> input;
  final ToolCallStatus status;
  final DateTime createdAt;
  final DateTime? startedAt;
  final DateTime? completedAt;

  const ToolCall({
    required this.id,
    required this.sessionId,
    required this.toolName,
    required this.input,
    required this.status,
    required this.createdAt,
    this.startedAt,
    this.completedAt,
  });

  factory ToolCall.fromJson(Map<String, dynamic> json) => ToolCall(
        id: json['id'] as String,
        sessionId: json['session_id'] as String,
        toolName: json['tool_name'] as String,
        input: (json['input'] as Map<String, dynamic>?) ?? const {},
        status: ToolCallStatus.values.byName(json['status'] as String),
        createdAt: DateTime.parse(json['created_at'] as String),
        startedAt: json['started_at'] != null
            ? DateTime.parse(json['started_at'] as String)
            : null,
        completedAt: json['completed_at'] != null
            ? DateTime.parse(json['completed_at'] as String)
            : null,
      );
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
        toolCallId: json['tool_call_id'] as String,
        isError: json['is_error'] as bool,
        output: json['output'],
      );
}
