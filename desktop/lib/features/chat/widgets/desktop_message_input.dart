import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

class DesktopMessageInput extends ConsumerWidget {
  const DesktopMessageInput({
    super.key,
    required this.sessionId,
    required this.session,
  });

  final String sessionId;
  final Session session;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final isLoading = session.status == SessionStatus.running;
    final isError = session.status == SessionStatus.error;

    if (isError) {
      return Container(
        padding: const EdgeInsets.all(12),
        decoration: const BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          border: Border(top: BorderSide(color: ClawdTheme.surfaceBorder)),
        ),
        child: Row(
          children: [
            const Icon(Icons.warning_amber, color: ClawdTheme.warning, size: 16),
            const SizedBox(width: 8),
            const Expanded(
              child: Text(
                'This session encountered an error.',
                style: TextStyle(fontSize: 13, color: ClawdTheme.warning),
              ),
            ),
            TextButton(
              onPressed: () =>
                  ref.read(sessionListProvider.notifier).resume(sessionId),
              child: const Text('Resume'),
            ),
          ],
        ),
      );
    }

    return MessageInput(
      isLoading: isLoading,
      hint: 'Message the AI… (Enter to send, Shift+Enter for newline)',
      onSend: (text) =>
          ref.read(messageListProvider(sessionId).notifier).send(text),
    );
  }
}
