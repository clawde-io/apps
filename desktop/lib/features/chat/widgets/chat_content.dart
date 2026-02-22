import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/features/chat/widgets/session_header.dart';
import 'package:clawde/features/chat/widgets/message_list.dart';
import 'package:clawde/features/chat/widgets/desktop_message_input.dart';
import 'package:clawde/features/chat/widgets/tool_call_panel.dart';

class ChatContent extends ConsumerWidget {
  const ChatContent({super.key});

  String _repoName(String path) {
    final parts = path.replaceAll(r'\', '/').split('/');
    return parts.where((p) => p.isNotEmpty).lastOrNull ?? path;
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final session = ref.watch(activeSessionProvider);

    // Update window title when active session changes (DP-03).
    ref.listen(activeSessionProvider, (_, next) {
      final title =
          next != null ? 'ClawDE — ${_repoName(next.repoPath)}' : 'ClawDE';
      windowManager.setTitle(title);
    });

    if (session == null) {
      return const EmptyState(
        icon: Icons.chat_bubble_outline,
        title: 'Select a session',
        subtitle: 'Choose from the sidebar or create a new one',
      );
    }

    return Column(
      children: [
        SessionHeader(session: session),
        Expanded(child: MessageList(sessionId: session.id)),
        ToolCallPanel(sessionId: session.id),
        DesktopMessageInput(sessionId: session.id, session: session),
      ],
    );
  }
}
