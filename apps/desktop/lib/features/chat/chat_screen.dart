import 'package:flutter/material.dart';
import 'package:clawde/features/chat/chat_layout.dart';
import 'package:clawde/features/chat/widgets/session_sidebar.dart';
import 'package:clawde/features/chat/widgets/chat_content.dart';
import 'package:clawde/features/chat/widgets/new_session_dialog.dart';

class ChatScreen extends StatelessWidget {
  const ChatScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return ChatLayout(
      sidebar: SessionSidebar(
        onNewSession: () => showDialog<void>(
          context: context,
          builder: (_) => const NewSessionDialog(),
        ),
      ),
      content: const ChatContent(),
    );
  }
}
