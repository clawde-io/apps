import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

class MessageList extends ConsumerStatefulWidget {
  const MessageList({super.key, required this.sessionId});

  final String sessionId;

  @override
  ConsumerState<MessageList> createState() => _MessageListState();
}

class _MessageListState extends ConsumerState<MessageList> {
  final _scrollController = ScrollController();

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  void _scrollToBottom() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scrollController.hasClients) {
        _scrollController.animateTo(
          _scrollController.position.maxScrollExtent,
          duration: const Duration(milliseconds: 300),
          curve: Curves.easeOut,
        );
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final messagesAsync = ref.watch(messageListProvider(widget.sessionId));

    ref.listen(messageListProvider(widget.sessionId), (prev, next) {
      final prevCount = prev?.valueOrNull?.length ?? 0;
      final nextCount = next.valueOrNull?.length ?? 0;
      if (nextCount > prevCount) _scrollToBottom();
    });

    return messagesAsync.when(
      loading: () => const _SkeletonMessages(),
      error: (e, _) => ErrorState(
        icon: Icons.error_outline,
        title: 'Could not load messages',
        description: e.toString(),
        onRetry: () =>
            ref.refresh(messageListProvider(widget.sessionId)),
      ),
      data: (messages) {
        if (messages.isEmpty) {
          return const EmptyState(
            icon: Icons.chat_bubble_outline,
            title: 'No messages yet',
            subtitle: 'Send a message below',
          );
        }
        return ListView.builder(
          controller: _scrollController,
          padding: const EdgeInsets.symmetric(vertical: 8),
          itemCount: messages.length,
          itemBuilder: (context, i) => ChatBubble(message: messages[i]),
        );
      },
    );
  }
}

class _SkeletonMessages extends StatelessWidget {
  const _SkeletonMessages();

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: const [
        _SkeletonBubble(isUser: false, width: 260),
        SizedBox(height: 8),
        _SkeletonBubble(isUser: true, width: 180),
        SizedBox(height: 8),
        _SkeletonBubble(isUser: false, width: 320),
      ],
    );
  }
}

class _SkeletonBubble extends StatelessWidget {
  const _SkeletonBubble({required this.isUser, required this.width});
  final bool isUser;
  final double width;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: isUser ? Alignment.centerRight : Alignment.centerLeft,
      child: Container(
        width: width,
        height: 48,
        margin: const EdgeInsets.symmetric(horizontal: 16),
        decoration: BoxDecoration(
          color: Colors.white10,
          borderRadius: BorderRadius.circular(12),
        ),
      ),
    );
  }
}
