import 'package:clawd_core/clawd_core.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

// ─── Provider ─────────────────────────────────────────────────────────────────

final _attentionMapProvider =
    FutureProvider.autoDispose.family<List<Map<String, dynamic>>, String>(
  (ref, sessionId) async {
    final client = ref.read(daemonProvider.notifier).client;
    final result = await client.call<Map<String, dynamic>>(
      'session.attentionMap',
      {'sessionId': sessionId, 'topN': 30},
    );
    final files = result['files'] as List<dynamic>? ?? [];
    return files.cast<Map<String, dynamic>>();
  },
);

// ─── Page ─────────────────────────────────────────────────────────────────────

class AttentionMapPage extends ConsumerWidget {
  const AttentionMapPage({super.key, required this.sessionId});

  final String sessionId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(_attentionMapProvider(sessionId));

    return Scaffold(
      appBar: AppBar(
        title: const Text('Attention Map'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: () => ref.invalidate(_attentionMapProvider(sessionId)),
          ),
        ],
      ),
      body: state.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('Error: $e')),
        data: (files) => files.isEmpty
            ? const _EmptyState()
            : _HeatmapView(files: files),
      ),
    );
  }
}

// ─── Heatmap view ─────────────────────────────────────────────────────────────

class _HeatmapView extends StatelessWidget {
  const _HeatmapView({required this.files});

  final List<Map<String, dynamic>> files;

  @override
  Widget build(BuildContext context) {
    final maxScore = files.fold<int>(
      1,
      (max, f) =>
          ((f['attentionScore'] as int? ?? 0) > max)
              ? (f['attentionScore'] as int)
              : max,
    );

    return ListView.builder(
      padding: const EdgeInsets.all(16),
      itemCount: files.length,
      itemBuilder: (ctx, i) {
        final file = files[i];
        final score = file['attentionScore'] as int? ?? 0;
        final heat = score / maxScore.toDouble();
        return _HeatmapTile(file: file, heat: heat, rank: i + 1);
      },
    );
  }
}

class _HeatmapTile extends StatelessWidget {
  const _HeatmapTile({
    required this.file,
    required this.heat,
    required this.rank,
  });

  final Map<String, dynamic> file;
  final double heat;
  final int rank;

  Color _heatColor() {
    // Interpolate: cool (blue) → warm (orange) → hot (red)
    if (heat >= 0.8) return Colors.red.shade700;
    if (heat >= 0.5) return Colors.orange.shade600;
    if (heat >= 0.3) return Colors.yellow.shade700;
    return Colors.blue.shade300;
  }

  @override
  Widget build(BuildContext context) {
    final filePath = file['filePath'] as String? ?? '';
    final readCount = file['readCount'] as int? ?? 0;
    final writeCount = file['writeCount'] as int? ?? 0;
    final mentionCount = file['mentionCount'] as int? ?? 0;
    final color = _heatColor();

    return Container(
      margin: const EdgeInsets.only(bottom: 8),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(8),
        child: Stack(
          children: [
            // Heat bar background.
            Positioned.fill(
              child: Align(
                alignment: Alignment.centerLeft,
                child: FractionallySizedBox(
                  widthFactor: heat.clamp(0.02, 1.0),
                  child: Container(color: color.withValues(alpha: 0.12)),
                ),
              ),
            ),
            // Content.
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
              child: Row(
                children: [
                  Text(
                    '#$rank',
                    style: Theme.of(context).textTheme.labelSmall?.copyWith(
                          color: color,
                          fontWeight: FontWeight.bold,
                        ),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          filePath.split('/').last,
                          style: Theme.of(context).textTheme.bodyMedium,
                          overflow: TextOverflow.ellipsis,
                        ),
                        Text(
                          filePath,
                          style: Theme.of(context)
                              .textTheme
                              .labelSmall
                              ?.copyWith(
                                  color: Theme.of(context).colorScheme.outline),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(width: 8),
                  _CountPill(icon: Icons.visibility, count: readCount, color: Colors.blue),
                  const SizedBox(width: 4),
                  _CountPill(icon: Icons.edit, count: writeCount, color: Colors.orange),
                  const SizedBox(width: 4),
                  _CountPill(
                      icon: Icons.chat_bubble_outline,
                      count: mentionCount,
                      color: Colors.purple),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _CountPill extends StatelessWidget {
  const _CountPill({
    required this.icon,
    required this.count,
    required this.color,
  });

  final IconData icon;
  final int count;
  final Color color;

  @override
  Widget build(BuildContext context) {
    if (count == 0) return const SizedBox.shrink();
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 10, color: color),
          const SizedBox(width: 2),
          Text(
            '$count',
            style: Theme.of(context)
                .textTheme
                .labelSmall
                ?.copyWith(color: color, fontWeight: FontWeight.bold),
          ),
        ],
      ),
    );
  }
}

class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.thermostat_outlined,
              size: 64, color: Theme.of(context).colorScheme.outline),
          const SizedBox(height: 16),
          Text('No attention data yet',
              style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          const Text(
            'File attention is tracked as the AI reads, writes,\nand mentions files during the session.',
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }
}
