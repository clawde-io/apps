import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Full-screen chat view for a single session on mobile.
/// ME-03: Pagination (load more on scroll-up)
/// ME-04: Overflow action menu
/// ME-05: Auto-scroll to latest message
/// ME-06: Markdown rendering via ChatBubble -> MarkdownMessage (in clawd_ui)
/// 42h-3: Inline tool call cards, file change cards, swipe approve/reject
class SessionDetailScreen extends ConsumerStatefulWidget {
  const SessionDetailScreen({super.key, required this.sessionId});

  final String sessionId;

  @override
  ConsumerState<SessionDetailScreen> createState() =>
      _SessionDetailScreenState();
}

class _SessionDetailScreenState extends ConsumerState<SessionDetailScreen> {
  final ScrollController _scrollController = ScrollController();
  bool _loadingMore = false;

  @override
  void initState() {
    super.initState();
    _scrollController.addListener(_onScroll);

    // ME-05: react to new messages and auto-scroll.
    // Called in initState so the listener is registered only once, not on
    // every rebuild.
    ref.listen(messageListProvider(widget.sessionId), (prev, next) {
      final prevCount = prev?.valueOrNull?.length ?? 0;
      final nextCount = next.valueOrNull?.length ?? 0;
      if (nextCount > prevCount) {
        _scrollToBottom();
      }
    });
  }

  @override
  void dispose() {
    _scrollController.removeListener(_onScroll);
    _scrollController.dispose();
    super.dispose();
  }

  void _onScroll() {
    // Load more when within 100px of the top
    if (_scrollController.position.pixels <=
            _scrollController.position.minScrollExtent + 100 &&
        !_loadingMore) {
      _loadMore();
    }
  }

  Future<void> _loadMore() async {
    setState(() => _loadingMore = true);
    try {
      await ref
          .read(messageListProvider(widget.sessionId).notifier)
          .loadMore();
    } finally {
      if (mounted) setState(() => _loadingMore = false);
    }
  }

  void _scrollToBottom() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scrollController.hasClients) {
        _scrollController.animateTo(
          _scrollController.position.maxScrollExtent,
          duration: const Duration(milliseconds: 200),
          curve: Curves.easeOut,
        );
      }
    });
  }

  Session? _findSession() => ref
      .watch(sessionListProvider)
      .valueOrNull
      ?.where((s) => s.id == widget.sessionId)
      .firstOrNull;

  @override
  Widget build(BuildContext context) {
    final session = _findSession();

    final messagesAsync = ref.watch(messageListProvider(widget.sessionId));
    final toolCallsAsync = ref.watch(toolCallProvider(widget.sessionId));

    return Scaffold(
      appBar: AppBar(
        title: Text(
          session?.repoPath.split('/').last ?? 'Session',
          style: const TextStyle(fontSize: 16),
        ),
        actions: [
          if (session != null)
            Padding(
              padding: const EdgeInsets.only(right: 4),
              child: ProviderBadge(provider: session.provider),
            ),
          // ME-04: overflow action menu
          if (session != null)
            PopupMenuButton<_SessionAction>(
              onSelected: (action) => _handleAction(context, action, session),
              itemBuilder: (_) => [
                if (session.status == SessionStatus.running)
                  const PopupMenuItem(
                    value: _SessionAction.pause,
                    child: ListTile(
                      leading: Icon(Icons.pause),
                      title: Text('Pause'),
                      contentPadding: EdgeInsets.zero,
                      dense: true,
                    ),
                  ),
                if (session.status == SessionStatus.paused)
                  const PopupMenuItem(
                    value: _SessionAction.resume,
                    child: ListTile(
                      leading: Icon(Icons.play_arrow),
                      title: Text('Resume'),
                      contentPadding: EdgeInsets.zero,
                      dense: true,
                    ),
                  ),
                const PopupMenuItem(
                  value: _SessionAction.close,
                  child: ListTile(
                    leading: Icon(Icons.stop_circle_outlined),
                    title: Text('Close session'),
                    contentPadding: EdgeInsets.zero,
                    dense: true,
                  ),
                ),
                const PopupMenuItem(
                  value: _SessionAction.cancel,
                  child: ListTile(
                    leading: Icon(Icons.cancel_outlined, color: Colors.redAccent),
                    title: Text(
                      'Cancel',
                      style: TextStyle(color: Colors.redAccent),
                    ),
                    contentPadding: EdgeInsets.zero,
                    dense: true,
                  ),
                ),
                const PopupMenuItem(
                  value: _SessionAction.export,
                  child: ListTile(
                    leading: Icon(Icons.ios_share_outlined),
                    title: Text('Export'),
                    contentPadding: EdgeInsets.zero,
                    dense: true,
                  ),
                ),
                const PopupMenuDivider(),
                PopupMenuItem(
                  value: _SessionAction.copyId,
                  child: ListTile(
                    leading: const Icon(Icons.copy, size: 18),
                    title: Text(
                      'Copy ID: ${widget.sessionId.substring(0, 8)}...',
                    ),
                    contentPadding: EdgeInsets.zero,
                    dense: true,
                  ),
                ),
              ],
            ),
        ],
      ),
      body: Column(
        children: [
          // Session error banner (SH-04 prep)
          if (session?.status == SessionStatus.error)
            _ErrorBanner(
              onResume: () => ref
                  .read(sessionListProvider.notifier)
                  .resume(widget.sessionId),
            ),

          // Pending tool calls banner (touch-optimized with larger tap target)
          toolCallsAsync.whenOrNull(
            data: (calls) {
              final pending = calls
                  .where((tc) => tc.status == ToolCallStatus.pending)
                  .toList();
              return pending.isEmpty
                  ? const SizedBox.shrink()
                  : _ToolCallBanner(
                      sessionId: widget.sessionId,
                      count: pending.length,
                      onTap: () => _showToolCallSheet(context, pending),
                    );
            },
          ) ??
              const SizedBox.shrink(),

          // ME-03: loading-more indicator at top
          if (_loadingMore)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 8),
              child: Center(
                child: SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
              ),
            ),

          // Message list with inline tool calls and file edits
          Expanded(
            child: messagesAsync.when(
              loading: () =>
                  const Center(child: CircularProgressIndicator()),
              error: (e, _) => Center(child: Text('Error: $e')),
              data: (msgs) {
                if (msgs.isEmpty) {
                  return const Center(
                    child: Text(
                      'No messages yet.\nSend a message below.',
                      textAlign: TextAlign.center,
                      style: TextStyle(color: Colors.white38),
                    ),
                  );
                }

                // Build interleaved list: messages + inline tool calls
                final items = _buildInterleaved(msgs, toolCallsAsync);

                return ListView.builder(
                  controller: _scrollController,
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  itemCount: items.length,
                  itemBuilder: (context, i) =>
                      _buildItem(context, items[i]),
                );
              },
            ),
          ),

          // Input bar
          MessageInput(
            isLoading: session?.status == SessionStatus.running,
            enabled: session?.status != SessionStatus.error,
            onSend: (text) => ref
                .read(messageListProvider(widget.sessionId).notifier)
                .send(text),
          ),
        ],
      ),
    );
  }

  /// Build an interleaved list of messages and tool calls.
  /// Tool calls appear inline after the assistant message that triggered them.
  List<_ChatItem> _buildInterleaved(
    List<Message> messages,
    AsyncValue<List<ToolCall>> toolCallsAsync,
  ) {
    final toolCalls = toolCallsAsync.valueOrNull ?? [];
    final items = <_ChatItem>[];

    for (final msg in messages) {
      items.add(_ChatItem.message(msg));

      // Check if this message has associated tool calls (by messageId).
      final msgToolCalls =
          toolCalls.where((tc) => tc.messageId == msg.id).toList();
      for (final tc in msgToolCalls) {
        items.add(_ChatItem.toolCall(tc));
      }

      // Check for file edit metadata in message.
      final fileEdits = _extractFileEdits(msg);
      for (final edit in fileEdits) {
        items.add(_ChatItem.fileEdit(edit));
      }
    }

    // Also add tool calls not associated with any message (orphans).
    final usedToolCallIds =
        items.whereType<_ToolCallChatItem>().map((i) => i.toolCall.id).toSet();
    for (final tc in toolCalls) {
      if (!usedToolCallIds.contains(tc.id)) {
        items.add(_ChatItem.toolCall(tc));
      }
    }

    return items;
  }

  /// Extract file edit information from message metadata.
  List<_FileEditInfo> _extractFileEdits(Message msg) {
    final edits = <_FileEditInfo>[];
    final rawFiles = msg.metadata['files'];
    // M13: Guard against unexpected types before casting.
    if (rawFiles is! List<dynamic>) return edits;
    for (final f in rawFiles) {
      if (f is Map<String, dynamic>) {
        edits.add(_FileEditInfo(
          filePath: f['path'] as String? ?? '',
          operation: f['operation'] as String? ?? 'edit',
          linesAdded: f['linesAdded'] as int? ?? 0,
          linesRemoved: f['linesRemoved'] as int? ?? 0,
          diffContent: f['diff'] as String?,
        ));
      }
    }
    return edits;
  }

  Widget _buildItem(BuildContext context, _ChatItem item) {
    return switch (item) {
      _MessageChatItem(:final message) => ChatBubble(message: message),
      _ToolCallChatItem(:final toolCall) => _InlineToolCallCard(
          toolCall: toolCall,
          onApprove: () => ref
              .read(toolCallProvider(widget.sessionId).notifier)
              .approve(toolCall.id),
          onReject: () => ref
              .read(toolCallProvider(widget.sessionId).notifier)
              .reject(toolCall.id),
        ),
      _FileEditChatItem(:final edit) => Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 2),
          child: FileEditCard(
            filePath: edit.filePath,
            operation: edit.operation,
            linesAdded: edit.linesAdded,
            linesRemoved: edit.linesRemoved,
            diffContent: edit.diffContent,
          ),
        ),
    };
  }

  void _handleAction(
    BuildContext context,
    _SessionAction action,
    Session session,
  ) {
    final notifier = ref.read(sessionListProvider.notifier);
    switch (action) {
      case _SessionAction.pause:
        notifier.pause(session.id);
      case _SessionAction.resume:
        notifier.resume(session.id);
      case _SessionAction.close:
        _confirmClose(context, session, notifier);
      case _SessionAction.cancel:
        notifier.cancel(session.id);
      case _SessionAction.export:
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Export not yet available')),
        );
      case _SessionAction.copyId:
        Clipboard.setData(ClipboardData(text: session.id));
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Session ID copied')),
        );
    }
  }

  Future<void> _confirmClose(
    BuildContext context,
    Session session,
    SessionListNotifier notifier,
  ) async {
    final navigator = Navigator.of(context);
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Close session?'),
        content: const Text('History is preserved.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('Close',
                style: TextStyle(color: Colors.red)),
          ),
        ],
      ),
    );
    if (confirmed == true) {
      await notifier.close(session.id);
      if (mounted) navigator.pop();
    }
  }

  void _showToolCallSheet(BuildContext context, List<ToolCall> calls) {
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      builder: (_) =>
          _ToolCallSheet(sessionId: widget.sessionId),
    );
  }
}

enum _SessionAction { pause, resume, close, cancel, export, copyId }

// ── Chat item types ──────────────────────────────────────────────────────────

sealed class _ChatItem {
  const _ChatItem();
  factory _ChatItem.message(Message msg) = _MessageChatItem;
  factory _ChatItem.toolCall(ToolCall tc) = _ToolCallChatItem;
  factory _ChatItem.fileEdit(_FileEditInfo edit) = _FileEditChatItem;
}

class _MessageChatItem extends _ChatItem {
  const _MessageChatItem(this.message);
  final Message message;
}

class _ToolCallChatItem extends _ChatItem {
  const _ToolCallChatItem(this.toolCall);
  final ToolCall toolCall;
}

class _FileEditChatItem extends _ChatItem {
  const _FileEditChatItem(this.edit);
  final _FileEditInfo edit;
}

class _FileEditInfo {
  final String filePath;
  final String operation;
  final int linesAdded;
  final int linesRemoved;
  final String? diffContent;

  const _FileEditInfo({
    required this.filePath,
    required this.operation,
    required this.linesAdded,
    required this.linesRemoved,
    this.diffContent,
  });
}

// ── Error banner ──────────────────────────────────────────────────────────────

class _ErrorBanner extends StatelessWidget {
  const _ErrorBanner({required this.onResume});

  final VoidCallback onResume;

  @override
  Widget build(BuildContext context) {
    return Container(
      color: Colors.red.withValues(alpha: 0.12),
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Row(
        children: [
          const Icon(Icons.error_outline, size: 14, color: Colors.redAccent),
          const SizedBox(width: 8),
          const Expanded(
            child: Text(
              'Session encountered an error.',
              style: TextStyle(color: Colors.redAccent, fontSize: 12),
            ),
          ),
          TextButton(
            onPressed: onResume,
            style: TextButton.styleFrom(
              foregroundColor: Colors.redAccent,
              padding:
                  const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
            ),
            child: const Text('Resume', style: TextStyle(fontSize: 12)),
          ),
        ],
      ),
    );
  }
}

// ── Tool call banner ──────────────────────────────────────────────────────────

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
        // Touch-optimized: minimum 48dp tap target
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        child: Row(
          children: [
            const Icon(Icons.terminal, size: 16, color: ClawdTheme.warning),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                '$count tool call${count == 1 ? '' : 's'} awaiting approval',
                style: const TextStyle(
                  color: ClawdTheme.warning,
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                ),
              ),
            ),
            const Icon(
              Icons.chevron_right,
              color: ClawdTheme.warning,
              size: 18,
            ),
          ],
        ),
      ),
    );
  }
}

// ── Inline tool call card with swipe gestures ────────────────────────────────

class _InlineToolCallCard extends StatelessWidget {
  const _InlineToolCallCard({
    required this.toolCall,
    this.onApprove,
    this.onReject,
  });

  final ToolCall toolCall;
  final VoidCallback? onApprove;
  final VoidCallback? onReject;

  bool get _isPending => toolCall.status == ToolCallStatus.pending;

  @override
  Widget build(BuildContext context) {
    final card = ToolCallCard(
      toolCall: toolCall,
      onApprove: onApprove,
      onReject: onReject,
    );

    // Swipe gestures for pending tool calls: right = approve, left = reject.
    if (!_isPending) return card;

    return Dismissible(
      key: ValueKey('tc-${toolCall.id}'),
      direction: DismissDirection.horizontal,
      confirmDismiss: (direction) async {
        if (direction == DismissDirection.startToEnd) {
          // Swipe right = approve
          onApprove?.call();
          HapticFeedback.mediumImpact();
        } else {
          // Swipe left = reject
          onReject?.call();
          HapticFeedback.mediumImpact();
        }
        return false; // keep the card visible (status update comes via push)
      },
      // Right swipe background (approve)
      background: Container(
        margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
        decoration: BoxDecoration(
          color: ClawdTheme.success.withValues(alpha: 0.2),
          borderRadius: BorderRadius.circular(8),
        ),
        alignment: Alignment.centerLeft,
        padding: const EdgeInsets.only(left: 20),
        child: const Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.check_circle, color: ClawdTheme.success, size: 20),
            SizedBox(width: 8),
            Text(
              'Approve',
              style: TextStyle(
                color: ClawdTheme.success,
                fontWeight: FontWeight.w600,
                fontSize: 13,
              ),
            ),
          ],
        ),
      ),
      // Left swipe background (reject)
      secondaryBackground: Container(
        margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
        decoration: BoxDecoration(
          color: ClawdTheme.error.withValues(alpha: 0.2),
          borderRadius: BorderRadius.circular(8),
        ),
        alignment: Alignment.centerRight,
        padding: const EdgeInsets.only(right: 20),
        child: const Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              'Reject',
              style: TextStyle(
                color: ClawdTheme.error,
                fontWeight: FontWeight.w600,
                fontSize: 13,
              ),
            ),
            SizedBox(width: 8),
            Icon(Icons.cancel, color: ClawdTheme.error, size: 20),
          ],
        ),
      ),
      child: card,
    );
  }
}

// ── Tool call sheet ───────────────────────────────────────────────────────────

class _ToolCallSheet extends ConsumerWidget {
  const _ToolCallSheet({required this.sessionId});

  final String sessionId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final toolCalls = ref.watch(toolCallProvider(sessionId));
    final notifier = ref.read(toolCallProvider(sessionId).notifier);

    return DraggableScrollableSheet(
      initialChildSize: 0.6,
      maxChildSize: 0.9,
      minChildSize: 0.3,
      expand: false,
      builder: (_, controller) => Column(
        children: [
          // Drag handle
          Container(
            margin: const EdgeInsets.only(top: 8),
            width: 36,
            height: 4,
            decoration: BoxDecoration(
              color: Colors.white24,
              borderRadius: BorderRadius.circular(2),
            ),
          ),
          Padding(
            padding: const EdgeInsets.all(16),
            child: Row(
              children: [
                const Text(
                  'Pending Tool Calls',
                  style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
                ),
                const Spacer(),
                // Approve all button
                toolCalls.whenOrNull(
                  data: (calls) {
                    final pending = calls
                        .where((tc) =>
                            tc.status == ToolCallStatus.pending)
                        .toList();
                    if (pending.length < 2) return const SizedBox.shrink();
                    return TextButton.icon(
                      onPressed: () {
                        for (final tc in pending) {
                          notifier.approve(tc.id);
                        }
                        HapticFeedback.mediumImpact();
                      },
                      icon: const Icon(Icons.done_all, size: 16),
                      label: Text('Approve all (${pending.length})'),
                      style: TextButton.styleFrom(
                        foregroundColor: ClawdTheme.success,
                      ),
                    );
                  },
                ) ??
                    const SizedBox.shrink(),
              ],
            ),
          ),
          Expanded(
            child: toolCalls.when(
              loading: () =>
                  const Center(child: CircularProgressIndicator()),
              error: (e, _) => Center(child: Text('$e')),
              data: (calls) {
                final pending = calls
                    .where((tc) => tc.status == ToolCallStatus.pending)
                    .toList();
                if (pending.isEmpty) {
                  return const Center(
                    child: Text(
                      'No pending tool calls.',
                      style: TextStyle(color: Colors.white38),
                    ),
                  );
                }
                return ListView.builder(
                  controller: controller,
                  itemCount: pending.length,
                  itemBuilder: (_, i) => _InlineToolCallCard(
                    toolCall: pending[i],
                    onApprove: () => notifier.approve(pending[i].id),
                    onReject: () => notifier.reject(pending[i].id),
                  ),
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}
