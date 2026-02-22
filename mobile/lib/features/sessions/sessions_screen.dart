import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Lists all sessions. Supports pull-to-refresh, swipe actions, and status filtering.
class SessionsScreen extends ConsumerStatefulWidget {
  const SessionsScreen({super.key});

  @override
  ConsumerState<SessionsScreen> createState() => _SessionsScreenState();
}

class _SessionsScreenState extends ConsumerState<SessionsScreen> {
  SessionStatus? _filter; // null = All

  List<Session> _applyFilter(List<Session> sessions) {
    if (_filter == null) return sessions;
    return sessions.where((s) => s.status == _filter).toList();
  }

  Future<void> _refresh() async {
    await ref.read(sessionListProvider.notifier).refresh();
  }

  @override
  Widget build(BuildContext context) {
    final sessionsAsync = ref.watch(sessionListProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('ClawDE'),
        actions: const [
          Padding(
            padding: EdgeInsets.only(right: 12),
            child: ConnectionStatusIndicator(),
          ),
        ],
      ),
      body: sessionsAsync.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('Error: $e')),
        data: (list) {
          final visible = _applyFilter(list);
          return RefreshIndicator(
            onRefresh: _refresh,
            child: CustomScrollView(
              physics: const AlwaysScrollableScrollPhysics(),
              slivers: [
                // Filter chips
                SliverToBoxAdapter(
                  child: _FilterRow(
                    sessions: list,
                    filter: _filter,
                    onFilterChanged: (f) => setState(() => _filter = f),
                  ),
                ),
                // Session list or empty state
                if (visible.isEmpty)
                  SliverFillRemaining(
                    child: _EmptyState(
                      isFiltered: _filter != null,
                    ),
                  )
                else
                  SliverList(
                    delegate: SliverChildBuilderDelegate(
                      (context, i) => _SwipeableSessionTile(
                        session: visible[i],
                      ),
                      childCount: visible.length,
                    ),
                  ),
              ],
            ),
          );
        },
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: () => _showNewSessionSheet(context),
        backgroundColor: ClawdTheme.claw,
        child: const Icon(Icons.add, color: Colors.white),
      ),
    );
  }

  void _showNewSessionSheet(BuildContext context) {
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      builder: (_) => const _NewSessionSheet(),
    );
  }
}

// ── Filter row ────────────────────────────────────────────────────────────────

class _FilterRow extends StatelessWidget {
  const _FilterRow({
    required this.sessions,
    required this.filter,
    required this.onFilterChanged,
  });

  final List<Session> sessions;
  final SessionStatus? filter;
  final ValueChanged<SessionStatus?> onFilterChanged;

  int _count(SessionStatus? status) =>
      status == null ? sessions.length : sessions.where((s) => s.status == status).length;

  @override
  Widget build(BuildContext context) {
    final items = <(SessionStatus?, String)>[
      (null, 'All'),
      (SessionStatus.running, 'Running'),
      (SessionStatus.paused, 'Paused'),
      (SessionStatus.completed, 'Done'),
      (SessionStatus.error, 'Error'),
    ];

    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 4),
      child: Row(
        children: items.map((item) {
          final (status, label) = item;
          final selected = filter == status;
          final count = _count(status);
          return Padding(
            padding: const EdgeInsets.only(right: 8),
            child: FilterChip(
              label: Text('$label ($count)'),
              selected: selected,
              onSelected: (_) => onFilterChanged(status),
              selectedColor: ClawdTheme.claw.withValues(alpha: 0.25),
              checkmarkColor: ClawdTheme.clawLight,
              labelStyle: TextStyle(
                fontSize: 12,
                color: selected ? ClawdTheme.clawLight : Colors.white60,
              ),
              backgroundColor: ClawdTheme.surfaceElevated,
              side: BorderSide(
                color: selected ? ClawdTheme.claw : ClawdTheme.surfaceBorder,
              ),
            ),
          );
        }).toList(),
      ),
    );
  }
}

// ── Swipeable session tile ────────────────────────────────────────────────────

class _SwipeableSessionTile extends ConsumerWidget {
  const _SwipeableSessionTile({required this.session});

  final Session session;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final notifier = ref.read(sessionListProvider.notifier);

    return Dismissible(
      key: ValueKey(session.id),
      // Right swipe = pause / resume
      background: Container(
        color: session.status == SessionStatus.running
            ? Colors.amber.withValues(alpha: 0.8)
            : Colors.green.withValues(alpha: 0.8),
        alignment: Alignment.centerLeft,
        padding: const EdgeInsets.only(left: 20),
        child: Icon(
          session.status == SessionStatus.running
              ? Icons.pause
              : Icons.play_arrow,
          color: Colors.white,
        ),
      ),
      // Left swipe = close
      secondaryBackground: Container(
        color: Colors.red.withValues(alpha: 0.8),
        alignment: Alignment.centerRight,
        padding: const EdgeInsets.only(right: 20),
        child: const Icon(Icons.close, color: Colors.white),
      ),
      confirmDismiss: (direction) async {
        if (direction == DismissDirection.startToEnd) {
          // Pause or resume — never fully dismiss
          if (session.status == SessionStatus.running) {
            await notifier.pause(session.id);
          } else if (session.status == SessionStatus.paused) {
            await notifier.resume(session.id);
          }
          return false; // keep the tile
        }
        // Left swipe = close with confirmation
        return _confirmClose(context);
      },
      onDismissed: (_) => notifier.close(session.id),
      child: SessionListTile(
        session: session,
        onTap: () {
          // SH-05: Persist session ID so it can be restored on next launch.
          SharedPreferences.getInstance().then(
            (prefs) => prefs.setString('last_active_session_id', session.id),
          );
          context.push('/session/${session.id}');
        },
      ),
    );
  }

  Future<bool> _confirmClose(BuildContext context) async {
    return await showDialog<bool>(
          context: context,
          builder: (ctx) => AlertDialog(
            title: const Text('Close session?'),
            content: const Text(
              'The session will be stopped. History is preserved.',
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.pop(ctx, false),
                child: const Text('Cancel'),
              ),
              TextButton(
                onPressed: () => Navigator.pop(ctx, true),
                child: const Text(
                  'Close',
                  style: TextStyle(color: Colors.red),
                ),
              ),
            ],
          ),
        ) ??
        false;
  }
}

// ── Empty state ────────────────────────────────────────────────────────────────

class _EmptyState extends StatelessWidget {
  const _EmptyState({required this.isFiltered});

  final bool isFiltered;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.auto_awesome, size: 48, color: ClawdTheme.clawLight),
          const SizedBox(height: 16),
          Text(
            isFiltered ? 'No matching sessions' : 'No sessions yet',
            style: const TextStyle(fontSize: 18, fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 8),
          Text(
            isFiltered
                ? 'Clear the filter to see all sessions'
                : 'Tap + to start an AI session',
            style: TextStyle(color: Colors.white.withValues(alpha: 0.5)),
          ),
        ],
      ),
    );
  }
}

// ── New session sheet ─────────────────────────────────────────────────────────

class _NewSessionSheet extends ConsumerStatefulWidget {
  const _NewSessionSheet();

  @override
  ConsumerState<_NewSessionSheet> createState() => _NewSessionSheetState();
}

class _NewSessionSheetState extends ConsumerState<_NewSessionSheet> {
  final _repoController = TextEditingController();
  bool _loading = false;

  @override
  void dispose() {
    _repoController.dispose();
    super.dispose();
  }

  Future<void> _create() async {
    final path = _repoController.text.trim();
    if (path.isEmpty) return;
    setState(() => _loading = true);
    try {
      final session = await ref
          .read(sessionListProvider.notifier)
          .create(repoPath: path);
      if (mounted) {
        Navigator.pop(context);
        context.push('/session/${session.id}');
      }
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: EdgeInsets.fromLTRB(
        16,
        16,
        16,
        MediaQuery.viewInsetsOf(context).bottom + 16,
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'New session',
            style: TextStyle(fontSize: 18, fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 16),
          TextField(
            controller: _repoController,
            decoration: const InputDecoration(
              labelText: 'Repository path',
              hintText: '/Users/you/projects/my-app',
            ),
            autofocus: true,
          ),
          const SizedBox(height: 16),
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              onPressed: _loading ? null : _create,
              style: FilledButton.styleFrom(
                backgroundColor: ClawdTheme.claw,
              ),
              child: _loading
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(
                        strokeWidth: 2,
                        color: Colors.white,
                      ),
                    )
                  : const Text('Start session'),
            ),
          ),
        ],
      ),
    );
  }
}
