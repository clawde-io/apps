/// clawd_proto — JSON-RPC 2.0 protocol types for the clawd daemon.
///
/// Mirrors the 17 RPC methods and 7 push event types defined in the
/// ClawDE system specification. Also includes Phase 41 agent task types.
library clawd_proto;

export 'src/session.dart';
export 'src/message.dart';
export 'src/repo_status.dart';
export 'src/tool_call.dart';
export 'src/rpc.dart';
export 'src/agent_task.dart';
export 'src/agent_activity.dart';
export 'src/task_dtos.dart';
export 'src/task_events.dart';
