import 'package:flutter/material.dart';
import 'package:split_view/split_view.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Split-pane layout for the Chat section.
/// [sidebar] is the session list (~22% width, 220–380px).
/// [content] is the main chat area (remaining width).
class ChatLayout extends StatelessWidget {
  const ChatLayout({
    super.key,
    required this.sidebar,
    required this.content,
  });

  final Widget sidebar;
  final Widget content;

  @override
  Widget build(BuildContext context) {
    return SplitView(
      viewMode: SplitViewMode.Horizontal,
      gripSize: 4,
      gripColor: ClawdTheme.surfaceBorder,
      controller: SplitViewController(weights: [0.22, 0.78]),
      children: [
        ConstrainedBox(
          constraints: const BoxConstraints(minWidth: 220, maxWidth: 380),
          child: sidebar,
        ),
        content,
      ],
    );
  }
}
