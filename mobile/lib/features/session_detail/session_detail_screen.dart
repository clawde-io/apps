import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Full-screen chat view for a single session on mobile.
class SessionDetailScreen extends ConsumerWidget {
  const SessionDetailScreen({super.key, required this.sessionId});

  final String sessionId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final session = ref.watch(sessionListProvider).valueOrNull
        ?.where((s) => s.id == sessionId)
        .firstOrNull;
    final messages = ref.watch(messageListProvider(sessionId));
    final toolCalls = ref.watch(toolCallProvider(sessionId));

    return Scaffold(
      appBar: AppBar(
        title: Text(
          session?.repoPath.split('/').last ?? 'Session',
          style: const TextStyle(fontSize: 16),
        ),
        actions: [
          if (session != null)
            Padding(
              padding: const EdgeInsets.only(right: 12),
              child: ProviderBadge(provider: session.provider),
            ),
        ],
      ),
      body: Column(
        children: [
          // Pending tool calls banner
          toolCalls.whenOrNull(
            data: (calls) => calls.isEmpty
                ? const SizedBox.shrink()
                : _ToolCallBanner(
                    sessionId: sessionId,
                    count: calls.length,
                    onTap: () => _showToolCallSheet(context, ref, calls),
                  ),
          ) ?? const SizedBox.shrink(),

          // Message list
          Expanded(
            child: messages.when(
              loading: () =>
                  const Center(child: CircularProgressIndicator()),
              error: (e, _) => Center(child: Text('Error: $e')),
              data: (msgs) => ListView.builder(
                padding: const EdgeInsets.symmetric(vertical: 8),
                itemCount: msgs.length,
                itemBuilder: (_, i) => ChatBubble(message: msgs[i]),
              ),
            ),
          ),

          // Input bar
          MessageInput(
            isLoading: session?.status.name == 'running',
            onSend: (text) =>
                ref.read(messageListProvider(sessionId).notifier).send(text),
          ),
        ],
      ),
    );
  }

  void _showToolCallSheet(
    BuildContext context,
    WidgetRef ref,
    List<dynamic> calls,
  ) {
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      builder: (_) => _ToolCallSheet(sessionId: sessionId, ref: ref),
    );
  }
}

class _ToolCallBanner extends StatelessWidget {
  const _ToolCallBanner({
    required this.sessionId,
    required this.count,
    required this.onTap,
  });

  final String sessionId;
  final int count;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        color: ClawdTheme.warning.withValues(alpha: 0.15),
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
        child: Row(
          children: [
            const Icon(Icons.terminal, size: 14, color: ClawdTheme.warning),
            const SizedBox(width: 8),
            Text(
              '$count tool call${count == 1 ? '' : 's'} awaiting approval',
              style: const TextStyle(
                  color: ClawdTheme.warning, fontSize: 12),
            ),
            const Spacer(),
            const Icon(Icons.chevron_right,
                color: ClawdTheme.warning, size: 14),
          ],
        ),
      ),
    );
  }
}

class _ToolCallSheet extends ConsumerWidget {
  const _ToolCallSheet({required this.sessionId, required this.ref});

  final String sessionId;
  final WidgetRef ref;

  @override
  Widget build(BuildContext context, WidgetRef innerRef) {
    final toolCalls = innerRef.watch(toolCallProvider(sessionId));
    final notifier = innerRef.read(toolCallProvider(sessionId).notifier);

    return DraggableScrollableSheet(
      initialChildSize: 0.6,
      maxChildSize: 0.9,
      minChildSize: 0.3,
      expand: false,
      builder: (_, controller) => Column(
        children: [
          const Padding(
            padding: EdgeInsets.all(16),
            child: Text(
              'Pending Tool Calls',
              style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
            ),
          ),
          Expanded(
            child: toolCalls.when(
              loading: () =>
                  const Center(child: CircularProgressIndicator()),
              error: (e, _) => Center(child: Text('$e')),
              data: (calls) => ListView.builder(
                controller: controller,
                itemCount: calls.length,
                itemBuilder: (_, i) => ToolCallCard(
                  toolCall: calls[i],
                  onApprove: () => notifier.approve(calls[i].id),
                  onReject: () => notifier.reject(calls[i].id),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
