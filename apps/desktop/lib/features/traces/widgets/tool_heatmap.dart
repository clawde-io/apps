// tool_heatmap.dart — Tool call heatmap (Sprint PP OB.9).
//
// Displays a grid of tool call counts per hour as a colour-scaled heatmap.
// Colour: transparent (0) → amber → red (high).

import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';

final _toolHeatmapProvider =
    FutureProvider.autoDispose<List<int>>((ref) async {
  final client = ref.read(daemonProvider.notifier).client;
  // Last 24 hours, hourly rollup tool call counts
  final since = DateTime.now()
          .subtract(const Duration(hours: 24))
          .millisecondsSinceEpoch ~/
      1000;
  final result = await client.call('metrics.rollups', {'since': since});
  final list = result['rollups'] as List<dynamic>? ?? [];
  return list
      .map((r) => ((r as Map)['tool_calls'] as num?)?.toInt() ?? 0)
      .toList();
});

class ToolHeatmap extends ConsumerWidget {
  const ToolHeatmap({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final data = ref.watch(_toolHeatmapProvider);

    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: const Color(0xFF1A1A24),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: const Color(0xFF2A2A3C)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Tool Calls (24h hourly)',
            style: TextStyle(fontSize: 11, color: Colors.white54),
          ),
          const SizedBox(height: 8),
          data.when(
            loading: () => const SizedBox(
              height: 28,
              child: Center(child: CircularProgressIndicator(strokeWidth: 2)),
            ),
            error: (_, __) => const SizedBox.shrink(),
            data: (counts) {
              if (counts.isEmpty) {
                return const Text(
                  'No data yet',
                  style: TextStyle(color: Colors.white38, fontSize: 12),
                );
              }
              final maxVal = counts.reduce(math.max);
              return Wrap(
                spacing: 3,
                runSpacing: 3,
                children: counts.map((c) {
                  final intensity = maxVal == 0 ? 0.0 : c / maxVal;
                  final color = _heatColor(intensity);
                  return Tooltip(
                    message: '$c tool calls',
                    child: Container(
                      width: 18,
                      height: 18,
                      decoration: BoxDecoration(
                        color: color,
                        borderRadius: BorderRadius.circular(3),
                      ),
                    ),
                  );
                }).toList(),
              );
            },
          ),
        ],
      ),
    );
  }

  Color _heatColor(double intensity) {
    if (intensity == 0) return const Color(0xFF2A2A3C);
    if (intensity < 0.33) return const Color(0xFF7C3AED).withValues(alpha: intensity * 3);
    if (intensity < 0.66) return const Color(0xFFD97706).withValues(alpha: 0.7);
    return const Color(0xFFDC2626).withValues(alpha: 0.8 + intensity * 0.2);
  }
}
