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
/// ME-06: Markdown rendering via ChatBubble → MarkdownMessage (in clawd_ui)
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

    // ME-05: react to new messages and auto-scroll
    ref.listen(messageListProvider(widget.sessionId), (prev, next) {
      final prevCount = prev?.valueOrNull?.length ?? 0;
      final nextCount = next.valueOrNull?.length ?? 0;
      if (nextCount > prevCount) {
        _scrollToBottom();
      }
    });

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
                const PopupMenuDivider(),
                PopupMenuItem(
                  value: _SessionAction.copyId,
                  child: ListTile(
                    leading: const Icon(Icons.copy, size: 18),
                    title: Text(
                      'Copy ID: ${widget.sessionId.substring(0, 8)}…',
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

          // Pending tool calls banner
          toolCallsAsync.whenOrNull(
            data: (calls) => calls.isEmpty
                ? const SizedBox.shrink()
                : _ToolCallBanner(
                    sessionId: widget.sessionId,
                    count: calls.length,
                    onTap: () => _showToolCallSheet(context, calls),
                  ),
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

          // Message list
          Expanded(
            child: messagesAsync.when(
              loading: () =>
                  const Center(child: CircularProgressIndicator()),
              error: (e, _) => Center(child: Text('Error: $e')),
              data: (msgs) => msgs.isEmpty
                  ? const Center(
                      child: Text(
                        'No messages yet.\nSend a message below.',
                        textAlign: TextAlign.center,
                        style: TextStyle(color: Colors.white38),
                      ),
                    )
                  : ListView.builder(
                      controller: _scrollController,
                      padding: const EdgeInsets.symmetric(vertical: 8),
                      itemCount: msgs.length,
                      // ME-06: ChatBubble renders markdown via MarkdownMessage in clawd_ui
                      itemBuilder: (_, i) => ChatBubble(message: msgs[i]),
                    ),
            ),
          ),

          // Input bar — disabled when session is in error state
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

  void _showToolCallSheet(BuildContext context, List<dynamic> calls) {
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      builder: (_) =>
          _ToolCallSheet(sessionId: widget.sessionId),
    );
  }
}

enum _SessionAction { pause, resume, close, copyId }

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
            const Icon(
              Icons.chevron_right,
              color: ClawdTheme.warning,
              size: 14,
            ),
          ],
        ),
      ),
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
