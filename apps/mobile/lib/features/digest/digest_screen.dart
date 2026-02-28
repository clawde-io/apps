import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';

// ─── Provider ─────────────────────────────────────────────────────────────────

/// Sprint EE DD.2 — fetches today's daily digest.
final _digestProvider =
    FutureProvider.autoDispose<DailyDigest>((ref) async {
  final client = ref.read(daemonProvider.notifier).client;
  final raw = await client.call<Map<String, dynamic>>('digest.today', {});
  return DailyDigest.fromJson(raw);
});

// ─── Screen ───────────────────────────────────────────────────────────────────

/// Sprint EE DD.2 — Mobile Daily Digest screen.
///
/// Shows a daily summary card of session activity:
/// - Tasks completed / in progress
/// - Sessions run
/// - Top files
/// Each session card is expandable for details.
class DigestScreen extends ConsumerWidget {
  const DigestScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final digestAsync = ref.watch(_digestProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Daily Digest'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: () => ref.invalidate(_digestProvider),
          ),
        ],
      ),
      body: digestAsync.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(
          child: Text('Failed to load digest: $e',
              style: const TextStyle(color: Colors.redAccent)),
        ),
        data: (digest) => digest.hasActivity
            ? _DigestContent(digest: digest)
            : const Center(
                child: Text(
                  'No activity today yet.\nStart a session to see your digest.',
                  textAlign: TextAlign.center,
                  style: TextStyle(color: Colors.white54),
                ),
              ),
      ),
    );
  }
}

// ─── Content ──────────────────────────────────────────────────────────────────

class _DigestContent extends StatelessWidget {
  const _DigestContent({required this.digest});

  final DailyDigest digest;

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        _MetricsCard(metrics: digest.metrics),
        const SizedBox(height: 16),
        if (digest.sessions.isNotEmpty) ...[
          Text(
            'Sessions',
            style: Theme.of(context)
                .textTheme
                .titleSmall
                ?.copyWith(color: Colors.white70),
          ),
          const SizedBox(height: 8),
          ...digest.sessions.map((s) => _SessionCard(entry: s)),
        ],
      ],
    );
  }
}

// ─── Metrics Card ─────────────────────────────────────────────────────────────

class _MetricsCard extends StatelessWidget {
  const _MetricsCard({required this.metrics});

  final DigestMetrics metrics;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                const Icon(Icons.today, size: 18, color: Colors.white54),
                const SizedBox(width: 8),
                Text(
                  'Today',
                  style: Theme.of(context)
                      .textTheme
                      .titleMedium
                      ?.copyWith(color: Colors.white),
                ),
              ],
            ),
            const SizedBox(height: 12),
            Row(
              children: [
                _MetricChip(
                  label: 'Sessions',
                  value: '${metrics.sessionsRun}',
                  color: Colors.blue,
                ),
                const SizedBox(width: 8),
                _MetricChip(
                  label: 'Done',
                  value: '${metrics.tasksCompleted}',
                  color: Colors.green,
                ),
                const SizedBox(width: 8),
                _MetricChip(
                  label: 'In Progress',
                  value: '${metrics.tasksInProgress}',
                  color: Colors.orange,
                ),
              ],
            ),
            if (metrics.topFiles.isNotEmpty) ...[
              const SizedBox(height: 8),
              Text(
                metrics.topFiles.take(3).join(', '),
                style: const TextStyle(
                  fontSize: 11,
                  color: Colors.white38,
                  fontFamily: 'monospace',
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _MetricChip extends StatelessWidget {
  const _MetricChip({
    required this.label,
    required this.value,
    required this.color,
  });

  final String label;
  final String value;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: color.withValues(alpha: 0.3)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            value,
            style: TextStyle(
                fontSize: 20, fontWeight: FontWeight.w700, color: color),
          ),
          Text(
            label,
            style: const TextStyle(fontSize: 10, color: Colors.white54),
          ),
        ],
      ),
    );
  }
}

// ─── Session Card ─────────────────────────────────────────────────────────────

class _SessionCard extends StatefulWidget {
  const _SessionCard({required this.entry});

  final DigestEntry entry;

  @override
  State<_SessionCard> createState() => _SessionCardState();
}

class _SessionCardState extends State<_SessionCard> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final e = widget.entry;
    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: Column(
        children: [
          ListTile(
            dense: true,
            leading: const Icon(Icons.chat_bubble_outline, size: 18),
            title: Text(
              e.sessionTitle ?? e.sessionId.substring(0, 8),
              style: const TextStyle(fontSize: 13),
            ),
            subtitle: Text(
              '${e.provider} · ${e.messagesCount} messages · ${e.tasksCompleted} tasks',
              style: const TextStyle(fontSize: 11, color: Colors.white54),
            ),
            trailing: IconButton(
              icon: Icon(
                _expanded ? Icons.expand_less : Icons.expand_more,
                size: 18,
                color: Colors.white54,
              ),
              onPressed: () => setState(() => _expanded = !_expanded),
            ),
          ),
          if (_expanded && e.filesChanged.isNotEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text(
                    'Files changed:',
                    style: TextStyle(fontSize: 11, color: Colors.white54),
                  ),
                  const SizedBox(height: 4),
                  ...e.filesChanged.map(
                    (f) => Text(
                      f,
                      style: const TextStyle(
                        fontSize: 11,
                        color: Colors.white70,
                        fontFamily: 'monospace',
                      ),
                    ),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}
