/// ClawDE core — shared Riverpod providers for daemon connection, sessions,
/// messages, and tool calls. Both desktop and mobile import this package.
library clawd_core;

export 'src/providers/daemon_provider.dart';
export 'src/providers/session_provider.dart';
export 'src/providers/message_provider.dart';
export 'src/providers/tool_call_provider.dart';
export 'src/providers/repo_provider.dart';
export 'src/providers/settings_provider.dart';
export 'src/providers/task_provider.dart';
export 'src/utils/paths.dart';
export 'src/session_export.dart';

// Phase 43l — multi-agent UX providers
export 'src/providers/agent_provider.dart';
export 'src/providers/task_summary_provider.dart';

// Device pairing, project management, and connection state providers
export 'src/providers/project_provider.dart';
export 'src/providers/device_provider.dart';
export 'src/providers/connection_state_provider.dart';
