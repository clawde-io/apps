/// clawd_proto — JSON-RPC 2.0 protocol types for the clawd daemon.
///
/// Mirrors the 17 RPC methods and 7 push event types defined in the
/// ClawDE system specification.
library clawd_proto;

export 'src/session.dart';
export 'src/message.dart';
export 'src/repo_status.dart';
export 'src/tool_call.dart';
export 'src/rpc.dart';
