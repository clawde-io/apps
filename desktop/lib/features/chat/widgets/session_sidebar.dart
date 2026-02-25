import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

class SessionSidebar extends ConsumerWidget {
  const SessionSidebar({super.key, this.onNewSession});

  final VoidCallback? onNewSession;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final sessionAsync = ref.watch(sessionListProvider);
    final activeId = ref.watch(activeSessionIdProvider);

    // V02.T24 — drift notification badge; empty until daemon supports drift.list
    final driftItems =
        ref.watch(driftItemsProvider).valueOrNull ?? [];

    return Container(
      color: ClawdTheme.surfaceElevated,
      child: Column(
        children: [
          // Header
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 8, 8),
            child: Row(
              children: [
                const Expanded(
                  child: Text(
                    'Sessions',
                    style: TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                      color: ClawdTheme.clawLight,
                      letterSpacing: 0.5,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                // V02.T24 — drift badge (hidden when count is 0)
                if (driftItems.isNotEmpty) ...[
                  DriftBadge(
                    count: driftItems.length,
                    items: driftItems,
                  ),
                  const SizedBox(width: 4),
                ],
                IconButton(
                  icon: const Icon(Icons.add, size: 18),
                  onPressed: onNewSession,
                  tooltip: 'New Session',
                  color: ClawdTheme.clawLight,
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
                ),
              ],
            ),
          ),
          const Divider(height: 1),
          // Session list
          Expanded(
            child: sessionAsync.when(
              loading: () => _SkeletonList(),
              error: (e, _) => ErrorState(
                icon: Icons.cloud_off,
                title: 'Could not load sessions',
                description: e.toString(),
                onRetry: () =>
                    ref.read(sessionListProvider.notifier).refresh(),
              ),
              data: (sessions) {
                if (sessions.isEmpty) {
                  return const EmptyState(
                    icon: Icons.chat_bubble_outline,
                    title: 'No sessions',
                    subtitle: 'Tap + to start an AI session',
                  );
                }
                return ListView.builder(
                  itemCount: sessions.length,
                  itemBuilder: (context, i) {
                    final session = sessions[i];
                    return _SessionTileWithMenu(
                      session: session,
                      isSelected: session.id == activeId,
                      onTap: () => ref
                          .read(activeSessionIdProvider.notifier)
                          .state = session.id,
                    );
                  },
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}

class _SessionTileWithMenu extends ConsumerWidget {
  const _SessionTileWithMenu({
    required this.session,
    required this.isSelected,
    required this.onTap,
  });

  final Session session;
  final bool isSelected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return GestureDetector(
      onSecondaryTapDown: (details) =>
          _showContextMenu(context, ref, details.globalPosition),
      child: SessionListTile(
        session: session,
        isSelected: isSelected,
        onTap: onTap,
      ),
    );
  }

  void _showContextMenu(
      BuildContext context, WidgetRef ref, Offset position) async {
    final notifier = ref.read(sessionListProvider.notifier);
    final activeNotifier = ref.read(activeSessionIdProvider.notifier);

    final result = await showMenu<_MenuAction>(
      context: context,
      position: RelativeRect.fromLTRB(
          position.dx, position.dy, position.dx + 1, position.dy + 1),
      items: [
        if (session.status == SessionStatus.running)
          const PopupMenuItem(
            value: _MenuAction.pause,
            child: Text('Pause'),
          ),
        if (session.status == SessionStatus.paused)
          const PopupMenuItem(
            value: _MenuAction.resume,
            child: Text('Resume'),
          ),
        const PopupMenuDivider(),
        const PopupMenuItem(value: _MenuAction.close, child: Text('Close')),
        const PopupMenuItem(
          value: _MenuAction.delete,
          child: Text('Delete', style: TextStyle(color: Colors.red)),
        ),
      ],
    );

    if (result == null || !context.mounted) return;
    switch (result) {
      case _MenuAction.pause:
        await notifier.pause(session.id);
      case _MenuAction.resume:
        await notifier.resume(session.id);
      case _MenuAction.close:
        await notifier.close(session.id);
        if (ref.read(activeSessionIdProvider) == session.id) {
          activeNotifier.state = null;
        }
      case _MenuAction.delete:
        final confirmed = await showDialog<bool>(
          context: context,
          builder: (ctx) => AlertDialog(
            title: const Text('Delete Session'),
            content: const Text(
                'This will permanently delete this session and all its messages.'),
            actions: [
              TextButton(
                onPressed: () => Navigator.pop(ctx, false),
                child: const Text('Cancel'),
              ),
              TextButton(
                onPressed: () => Navigator.pop(ctx, true),
                child: const Text('Delete',
                    style: TextStyle(color: Colors.red)),
              ),
            ],
          ),
        );
        if (confirmed == true) {
          await notifier.delete(session.id);
          if (ref.read(activeSessionIdProvider) == session.id) {
            activeNotifier.state = null;
          }
        }
    }
  }
}

enum _MenuAction { pause, resume, close, delete }

class _SkeletonList extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return ListView.separated(
      itemCount: 3,
      separatorBuilder: (_, __) => const Divider(height: 1),
      itemBuilder: (_, __) => const ListTile(
        leading: CircleAvatar(radius: 4, backgroundColor: Colors.white12),
        title: _SkeletonBox(width: 100, height: 12),
        subtitle: _SkeletonBox(width: 140, height: 10),
      ),
    );
  }
}

class _SkeletonBox extends StatelessWidget {
  const _SkeletonBox({required this.width, required this.height});
  final double width;
  final double height;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: width,
      height: height,
      decoration: BoxDecoration(
        color: Colors.white10,
        borderRadius: BorderRadius.circular(4),
      ),
    );
  }
}
