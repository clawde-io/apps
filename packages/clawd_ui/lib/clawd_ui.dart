/// ClawDE shared widget library. Both desktop and mobile import this.
/// Widgets here are platform-agnostic — layout adaptation is the app's job.
library clawd_ui;

export 'src/theme/clawd_theme.dart';
export 'src/widgets/chat_bubble.dart';
export 'src/widgets/session_list_tile.dart';
export 'src/widgets/tool_call_card.dart';
export 'src/widgets/message_input.dart';
export 'src/widgets/connection_status_indicator.dart';
export 'src/widgets/provider_badge.dart';
export 'src/widgets/markdown_message.dart';
export 'src/widgets/error_state.dart';
export 'src/widgets/empty_state.dart';
export 'src/widgets/task_status_badge.dart';
export 'src/widgets/agent_chip.dart';
export 'src/widgets/task_card.dart';
export 'src/widgets/kanban_column.dart';
export 'src/widgets/kanban_board.dart';
export 'src/widgets/activity_feed_item.dart';
export 'src/widgets/activity_feed.dart';
export 'src/widgets/task_detail_panel.dart';
// Phase 41 — agent dashboard widgets
export 'src/widgets/agent_swimlane_row.dart';
export 'src/widgets/filter_bar.dart';
export 'src/widgets/add_task_dialog.dart';
export 'src/widgets/project_selector.dart';
export 'src/widgets/phase_log_view.dart';
export 'src/widgets/phase_indicator.dart';
export 'src/widgets/file_edit_card.dart';
export 'src/widgets/context_budget_bar.dart';

// Phase 43l — multi-agent UX widgets
export 'src/widgets/agent_feed.dart';
export 'src/widgets/worktree_status.dart';
export 'src/widgets/approval_card.dart';
