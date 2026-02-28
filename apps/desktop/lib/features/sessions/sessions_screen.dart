import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:go_router/go_router.dart';
import 'package:clawde/router.dart';
import 'package:clawde/features/chat/widgets/new_session_dialog.dart';

enum _SortOrder { recent, oldest, byStatus }

class SessionsScreen extends ConsumerStatefulWidget {
  const SessionsScreen({super.key});

  @override
  ConsumerState<SessionsScreen> createState() => _SessionsScreenState();
}

class _SessionsScreenState extends ConsumerState<SessionsScreen> {
  SessionStatus? _filter; // null = All
  _SortOrder _sort = _SortOrder.recent;

  List<Session> _applyFilterAndSort(List<Session> sessions) {
    // Filter
    final filtered = _filter == null
        ? sessions
        : sessions.where((s) => s.status == _filter).toList();

    // Sort
    switch (_sort) {
      case _SortOrder.recent:
        return filtered; // daemon returns newest first
      case _SortOrder.oldest:
        return filtered.reversed.toList();
      case _SortOrder.byStatus:
        const order = [
          SessionStatus.running,
          SessionStatus.paused,
          SessionStatus.idle,
          SessionStatus.completed,
          SessionStatus.error,
        ];
        return [...filtered]..sort(
            (a, b) => order.indexOf(a.status) - order.indexOf(b.status));
    }
  }

  int _countForStatus(List<Session> sessions, SessionStatus status) =>
      sessions.where((s) => s.status == status).length;

  @override
  Widget build(BuildContext context) {
    final daemonState = ref.watch(daemonProvider);
    final sessionsAsync = ref.watch(sessionListProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // ── Header ──────────────────────────────────────────────────────────
        Container(
          height: 56,
          padding: const EdgeInsets.symmetric(horizontal: 20),
          decoration: const BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            border: Border(
              bottom: BorderSide(color: ClawdTheme.surfaceBorder),
            ),
          ),
          child: Row(
            children: [
              const Text(
                'Sessions',
                style: TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  color: Colors.white,
                ),
              ),
              const SizedBox(width: 8),
              sessionsAsync.when(
                data: (sessions) => Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                  decoration: BoxDecoration(
                    color: ClawdTheme.claw.withValues(alpha: 0.2),
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Text(
                    '${sessions.length}',
                    style: const TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w600,
                      color: ClawdTheme.clawLight,
                    ),
                  ),
                ),
                loading: () => const SizedBox.shrink(),
                error: (_, __) => const SizedBox.shrink(),
              ),
              const Spacer(),
              // Sort dropdown
              PopupMenuButton<_SortOrder>(
                initialValue: _sort,
                tooltip: 'Sort order',
                icon: const Icon(Icons.sort, size: 18, color: Colors.white54),
                color: ClawdTheme.surfaceElevated,
                onSelected: (v) => setState(() => _sort = v),
                itemBuilder: (_) => const [
                  PopupMenuItem(
                    value: _SortOrder.recent,
                    child: Text('Recent first'),
                  ),
                  PopupMenuItem(
                    value: _SortOrder.oldest,
                    child: Text('Oldest first'),
                  ),
                  PopupMenuItem(
                    value: _SortOrder.byStatus,
                    child: Text('By status'),
                  ),
                ],
              ),
              const SizedBox(width: 4),
              FilledButton.icon(
                onPressed: () => showDialog(
                  context: context,
                  builder: (_) => const NewSessionDialog(),
                ),
                icon: const Icon(Icons.add, size: 16),
                label: const Text('New Session'),
                style: FilledButton.styleFrom(
                  backgroundColor: ClawdTheme.claw,
                  foregroundColor: Colors.white,
                  padding:
                      const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
                ),
              ),
            ],
          ),
        ),

        // ── Daemon info card ────────────────────────────────────────────────
        _DaemonInfoCard(daemonState: daemonState),

        // ── Filter chips ────────────────────────────────────────────────────
        sessionsAsync.when(
          data: (sessions) => _FilterChips(
            sessions: sessions,
            filter: _filter,
            onFilterChanged: (f) => setState(() => _filter = f),
            countForStatus: _countForStatus,
          ),
          loading: () => const SizedBox.shrink(),
          error: (_, __) => const SizedBox.shrink(),
        ),

        // ── Session list ────────────────────────────────────────────────────
        Expanded(
          child: sessionsAsync.when(
            loading: () => const Center(child: CircularProgressIndicator()),
            error: (e, _) => ErrorState(
              icon: Icons.error_outline,
              title: 'Failed to load sessions',
              description: e.toString(),
              onRetry: () => ref.refresh(sessionListProvider),
            ),
            data: (sessions) {
              final visible = _applyFilterAndSort(sessions);
              if (visible.isEmpty) {
                return EmptyState(
                  icon: Icons.history,
                  title: _filter == null
                      ? 'No sessions yet'
                      : 'No ${_filter!.name} sessions',
                  subtitle: _filter == null
                      ? 'Tap "New Session" to start an AI coding session'
                      : 'Change the filter to see other sessions',
                );
              }
              return ListView.builder(
                padding: const EdgeInsets.symmetric(vertical: 8),
                itemCount: visible.length,
                itemBuilder: (context, i) => _SessionRow(session: visible[i]),
              );
            },
          ),
        ),
      ],
    );
  }
}

// ── Daemon info card ──────────────────────────────────────────────────────────

class _DaemonInfoCard extends ConsumerWidget {
  const _DaemonInfoCard({required this.daemonState});
  final DaemonState daemonState;

  String _formatUptime(int seconds) {
    if (seconds < 60) return '${seconds}s';
    final m = seconds ~/ 60;
    if (m < 60) return '${m}m';
    final h = m ~/ 60;
    final rem = m % 60;
    return rem > 0 ? '${h}h ${rem}m' : '${h}h';
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final info = daemonState.daemonInfo;

    return Container(
      margin: const EdgeInsets.fromLTRB(16, 12, 16, 0),
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Row(
        children: [
          Icon(
            daemonState.isConnected ? Icons.circle : Icons.circle_outlined,
            size: 8,
            color: daemonState.isConnected ? Colors.green : Colors.red,
          ),
          const SizedBox(width: 8),
          Text(
            daemonState.isConnected ? 'Daemon connected' : 'Disconnected',
            style: TextStyle(
              fontSize: 12,
              color: daemonState.isConnected ? Colors.green : Colors.red,
            ),
          ),
          if (info != null) ...[
            const SizedBox(width: 16),
            _InfoChip(
                label: 'v${info.version}', icon: Icons.info_outline, size: 12),
            const SizedBox(width: 8),
            _InfoChip(
              label: 'up ${_formatUptime(info.uptime)}',
              icon: Icons.timer_outlined,
              size: 12,
            ),
            const SizedBox(width: 8),
            _InfoChip(
              label: ':${info.port}',
              icon: Icons.settings_ethernet,
              size: 12,
            ),
          ],
          const Spacer(),
          if (!daemonState.isConnected)
            TextButton.icon(
              onPressed: () =>
                  ref.read(daemonProvider.notifier).reconnect(),
              icon: const Icon(Icons.refresh, size: 14),
              label: const Text('Reconnect'),
              style: TextButton.styleFrom(
                foregroundColor: ClawdTheme.clawLight,
                padding:
                    const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              ),
            ),
        ],
      ),
    );
  }
}

class _InfoChip extends StatelessWidget {
  const _InfoChip(
      {required this.label, required this.icon, required this.size});
  final String label;
  final IconData icon;
  final double size;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: size, color: Colors.white38),
        const SizedBox(width: 3),
        Text(label, style: const TextStyle(fontSize: 11, color: Colors.white38)),
      ],
    );
  }
}

// ── Filter chips ──────────────────────────────────────────────────────────────

class _FilterChips extends StatelessWidget {
  const _FilterChips({
    required this.sessions,
    required this.filter,
    required this.onFilterChanged,
    required this.countForStatus,
  });

  final List<Session> sessions;
  final SessionStatus? filter;
  final ValueChanged<SessionStatus?> onFilterChanged;
  final int Function(List<Session>, SessionStatus) countForStatus;

  @override
  Widget build(BuildContext context) {
    final statuses = [
      (null, 'All', sessions.length),
      (SessionStatus.running, 'Running',
          countForStatus(sessions, SessionStatus.running)),
      (SessionStatus.paused, 'Paused',
          countForStatus(sessions, SessionStatus.paused)),
      (SessionStatus.completed, 'Completed',
          countForStatus(sessions, SessionStatus.completed)),
      (SessionStatus.error, 'Error',
          countForStatus(sessions, SessionStatus.error)),
    ];

    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 10, 16, 4),
      child: Wrap(
        spacing: 8,
        children: statuses.map((item) {
          final (status, label, count) = item;
          final selected = filter == status;
          return FilterChip(
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
          );
        }).toList(),
      ),
    );
  }
}

// ── Session row ───────────────────────────────────────────────────────────────

class _SessionRow extends ConsumerWidget {
  const _SessionRow({required this.session});
  final Session session;

  Color _statusColor(SessionStatus s) => switch (s) {
        SessionStatus.running => Colors.green,
        SessionStatus.paused => Colors.amber,
        SessionStatus.idle => Colors.white38,
        SessionStatus.completed => Colors.teal,
        SessionStatus.error => Colors.red,
      };

  String _repoName(String path) {
    final parts = path.replaceAll(r'\', '/').split('/');
    return parts.where((p) => p.isNotEmpty).lastOrNull ?? path;
  }

  String _relativeTime(DateTime? dt) {
    if (dt == null) return '—';
    final diff = DateTime.now().difference(dt);
    if (diff.inSeconds < 60) return 'just now';
    if (diff.inMinutes < 60) return '${diff.inMinutes}m ago';
    if (diff.inHours < 24) return '${diff.inHours}h ago';
    return '${diff.inDays}d ago';
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final statusColor = _statusColor(session.status);

    return InkWell(
      onTap: () {
        ref.read(activeSessionIdProvider.notifier).state = session.id;
        context.go(routeChat);
      },
      child: Container(
        margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        decoration: BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: ClawdTheme.surfaceBorder),
        ),
        child: Row(
          children: [
            // Status dot
            Container(
              width: 8,
              height: 8,
              decoration: BoxDecoration(
                color: statusColor,
                shape: BoxShape.circle,
              ),
            ),
            const SizedBox(width: 12),

            // Repo name + path
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    _repoName(session.repoPath),
                    style: const TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                      color: Colors.white,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 2),
                  Text(
                    session.repoPath,
                    style: const TextStyle(fontSize: 11, color: Colors.white38),
                    overflow: TextOverflow.ellipsis,
                  ),
                ],
              ),
            ),
            const SizedBox(width: 12),

            // Provider badge
            ProviderBadge(provider: session.provider),
            const SizedBox(width: 12),

            // Status label + time
            Column(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Text(
                  session.status.name,
                  style: TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                    color: statusColor,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  _relativeTime(session.createdAt),
                  style:
                      const TextStyle(fontSize: 11, color: Colors.white38),
                ),
              ],
            ),
            const SizedBox(width: 12),

            // Action buttons
            _ActionButtons(session: session),
          ],
        ),
      ),
    );
  }
}

class _ActionButtons extends ConsumerStatefulWidget {
  const _ActionButtons({required this.session});
  final Session session;

  @override
  ConsumerState<_ActionButtons> createState() => _ActionButtonsState();
}

class _ActionButtonsState extends ConsumerState<_ActionButtons> {
  bool _isLoading = false;

  Future<void> _run(Future<void> Function() action) async {
    if (_isLoading) return;
    setState(() => _isLoading = true);
    try {
      await action();
    } finally {
      if (mounted) setState(() => _isLoading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final notifier = ref.read(sessionListProvider.notifier);
    final session = widget.session;

    if (_isLoading) {
      return const SizedBox(
        width: 20,
        height: 20,
        child: CircularProgressIndicator(strokeWidth: 2),
      );
    }

    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (session.status == SessionStatus.running)
          IconButton(
            icon: const Icon(Icons.pause, size: 16),
            tooltip: 'Pause',
            color: Colors.white54,
            onPressed: () => _run(() => notifier.pause(session.id)),
            padding: const EdgeInsets.all(4),
            constraints: const BoxConstraints(),
          ),
        if (session.status == SessionStatus.paused)
          IconButton(
            icon: const Icon(Icons.play_arrow, size: 16),
            tooltip: 'Resume',
            color: Colors.white54,
            onPressed: () => _run(() => notifier.resume(session.id)),
            padding: const EdgeInsets.all(4),
            constraints: const BoxConstraints(),
          ),
        // UI.7 — export session history to clipboard as markdown
        IconButton(
          icon: const Icon(Icons.download, size: 16),
          tooltip: 'Export to clipboard',
          color: Colors.white38,
          onPressed: () => _run(() async {
            // Capture messenger before async gap to avoid BuildContext warning.
            final messenger = ScaffoldMessenger.of(context);
            final messages =
                await ref.read(messageListProvider(session.id).future);
            final md = exportSessionToMarkdown(session, messages);
            await Clipboard.setData(ClipboardData(text: md));
            if (mounted) {
              messenger.showSnackBar(
                const SnackBar(
                  content: Text('Session exported to clipboard'),
                  duration: Duration(seconds: 2),
                ),
              );
            }
          }),
          padding: const EdgeInsets.all(4),
          constraints: const BoxConstraints(),
        ),
        IconButton(
          icon: const Icon(Icons.close, size: 16),
          tooltip: 'Close',
          color: Colors.white38,
          onPressed: () => _run(() => notifier.close(session.id)),
          padding: const EdgeInsets.all(4),
          constraints: const BoxConstraints(),
        ),
      ],
    );
  }
}
