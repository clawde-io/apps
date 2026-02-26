// SPDX-License-Identifier: MIT
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

// ─── Dead-letter provider ─────────────────────────────────────────────────────

/// Fetches dead-letter events from the daemon.
final deadLetterProvider = FutureProvider.autoDispose
    .family<List<Map<String, dynamic>>, String?>((ref, statusFilter) async {
  final client = ref.read(daemonProvider.notifier).client;
  final result = await client.call<Map<String, dynamic>>(
    'dead_letter.list',
    {
      'limit': 100,
      if (statusFilter != null) 'status': statusFilter,
    },
  );
  final events = result['events'] as List<dynamic>? ?? [];
  return events.cast<Map<String, dynamic>>();
});

// ─── Dead Letter Tab ──────────────────────────────────────────────────────────

/// Tab shown inside the Session Events panel displaying failed push events
/// with a per-row retry button.
class DeadLetterTab extends ConsumerStatefulWidget {
  const DeadLetterTab({super.key});

  @override
  ConsumerState<DeadLetterTab> createState() => _DeadLetterTabState();
}

class _DeadLetterTabState extends ConsumerState<DeadLetterTab> {
  String? _statusFilter; // null = all
  final _retrying = <String>{};

  @override
  Widget build(BuildContext context) {
    final eventsAsync = ref.watch(deadLetterProvider(_statusFilter));

    return Column(
      children: [
        // ── Toolbar ────────────────────────────────────────────────────────
        _Toolbar(
          statusFilter: _statusFilter,
          onFilterChanged: (v) => setState(() => _statusFilter = v),
          onRefresh: () => ref.invalidate(deadLetterProvider(_statusFilter)),
        ),
        // ── Events list ────────────────────────────────────────────────────
        Expanded(
          child: eventsAsync.when(
            loading: () =>
                const Center(child: CircularProgressIndicator(strokeWidth: 2)),
            error: (e, _) => _ErrorView(message: e.toString()),
            data: (events) => events.isEmpty
                ? const _EmptyView()
                : ListView.separated(
                    itemCount: events.length,
                    separatorBuilder: (_, __) =>
                        const Divider(height: 1, color: ClawdTheme.surfaceBorder),
                    itemBuilder: (_, i) => _EventTile(
                      event: events[i],
                      isRetrying: _retrying.contains(events[i]['id']),
                      onRetry: () => _retry(events[i]['id'] as String),
                    ),
                  ),
          ),
        ),
      ],
    );
  }

  Future<void> _retry(String id) async {
    if (_retrying.contains(id)) return;
    setState(() => _retrying.add(id));
    try {
      final client = ref.read(daemonProvider.notifier).client;
      await client.call<Map<String, dynamic>>(
        'dead_letter.retry',
        {'id': id},
      );
      ref.invalidate(deadLetterProvider(_statusFilter));
    } finally {
      if (mounted) setState(() => _retrying.remove(id));
    }
  }
}

// ─── Toolbar ─────────────────────────────────────────────────────────────────

class _Toolbar extends StatelessWidget {
  const _Toolbar({
    required this.statusFilter,
    required this.onFilterChanged,
    required this.onRefresh,
  });

  final String? statusFilter;
  final ValueChanged<String?> onFilterChanged;
  final VoidCallback onRefresh;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 40,
      padding: const EdgeInsets.symmetric(horizontal: 12),
      decoration: const BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        border: Border(bottom: BorderSide(color: ClawdTheme.surfaceBorder)),
      ),
      child: Row(
        children: [
          _FilterChip(
            label: 'All',
            selected: statusFilter == null,
            onTap: () => onFilterChanged(null),
          ),
          const SizedBox(width: 6),
          _FilterChip(
            label: 'Pending',
            selected: statusFilter == 'pending',
            onTap: () => onFilterChanged('pending'),
          ),
          const SizedBox(width: 6),
          _FilterChip(
            label: 'Failed',
            selected: statusFilter == 'permanently_failed',
            onTap: () => onFilterChanged('permanently_failed'),
          ),
          const Spacer(),
          IconButton(
            icon: const Icon(Icons.refresh, size: 16),
            color: Colors.white54,
            tooltip: 'Refresh',
            onPressed: onRefresh,
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
          ),
        ],
      ),
    );
  }
}

class _FilterChip extends StatelessWidget {
  const _FilterChip({
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
        decoration: BoxDecoration(
          color: selected
              ? ClawdTheme.claw.withValues(alpha: 0.2)
              : Colors.transparent,
          borderRadius: BorderRadius.circular(10),
          border: Border.all(
            color: selected
                ? ClawdTheme.claw.withValues(alpha: 0.5)
                : Colors.white.withValues(alpha: 0.15),
          ),
        ),
        child: Text(
          label,
          style: TextStyle(
            fontSize: 10,
            fontWeight: FontWeight.w600,
            color: selected ? ClawdTheme.clawLight : Colors.white54,
          ),
        ),
      ),
    );
  }
}

// ─── Event Tile ───────────────────────────────────────────────────────────────

class _EventTile extends StatelessWidget {
  const _EventTile({
    required this.event,
    required this.isRetrying,
    required this.onRetry,
  });

  final Map<String, dynamic> event;
  final bool isRetrying;
  final VoidCallback onRetry;

  String get _eventType => event['eventType'] as String? ?? '';
  String get _failureReason => event['failureReason'] as String? ?? '';
  int get _retryCount => (event['retryCount'] as num?)?.toInt() ?? 0;
  String get _status => event['status'] as String? ?? '';
  String? get _sessionId => event['sourceSessionId'] as String?;

  Color get _statusColor => switch (_status) {
        'permanently_failed' => const Color(0xFFef4444),
        'retrying' => ClawdTheme.warning,
        _ => const Color(0xFF60a5fa),
      };

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Status dot
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Container(
              width: 8,
              height: 8,
              decoration: BoxDecoration(
                color: _statusColor,
                shape: BoxShape.circle,
              ),
            ),
          ),
          const SizedBox(width: 10),
          // Content
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(
                      _eventType,
                      style: const TextStyle(
                        fontSize: 12,
                        fontWeight: FontWeight.w600,
                        color: Colors.white,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Container(
                      padding:
                          const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                      decoration: BoxDecoration(
                        color: _statusColor.withValues(alpha: 0.15),
                        borderRadius: BorderRadius.circular(4),
                      ),
                      child: Text(
                        _status.replaceAll('_', ' '),
                        style: TextStyle(
                          fontSize: 9,
                          fontWeight: FontWeight.w600,
                          color: _statusColor,
                        ),
                      ),
                    ),
                  ],
                ),
                if (_failureReason.isNotEmpty) ...[
                  const SizedBox(height: 3),
                  Text(
                    _failureReason,
                    style:
                        const TextStyle(fontSize: 11, color: Colors.white54),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                ],
                if (_sessionId != null) ...[
                  const SizedBox(height: 2),
                  Text(
                    'session: ${_sessionId!.substring(0, 8)}...',
                    style:
                        const TextStyle(fontSize: 10, color: Colors.white30),
                  ),
                ],
                Text(
                  'retries: $_retryCount',
                  style: const TextStyle(fontSize: 10, color: Colors.white30),
                ),
              ],
            ),
          ),
          // Retry button
          if (_status != 'retrying')
            TextButton(
              onPressed: isRetrying ? null : onRetry,
              style: TextButton.styleFrom(
                foregroundColor: const Color(0xFF60a5fa),
                padding:
                    const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                minimumSize: const Size(40, 28),
              ),
              child: isRetrying
                  ? const SizedBox(
                      width: 12,
                      height: 12,
                      child: CircularProgressIndicator(
                        strokeWidth: 1.5,
                        color: Color(0xFF60a5fa),
                      ),
                    )
                  : const Text('Retry', style: TextStyle(fontSize: 11)),
            ),
        ],
      ),
    );
  }
}

// ─── Empty & Error views ─────────────────────────────────────────────────────

class _EmptyView extends StatelessWidget {
  const _EmptyView();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.check_circle_outline,
              size: 36, color: Colors.white.withValues(alpha: 0.2)),
          const SizedBox(height: 10),
          Text(
            'No dead-letter events',
            style: TextStyle(
              fontSize: 13,
              color: Colors.white.withValues(alpha: 0.3),
            ),
          ),
        ],
      ),
    );
  }
}

class _ErrorView extends StatelessWidget {
  const _ErrorView({required this.message});
  final String message;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Text(
          'Error: $message',
          style: const TextStyle(fontSize: 12, color: Colors.white38),
        ),
      ),
    );
  }
}
