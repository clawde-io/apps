// token_burn_chart.dart — Token burn rate sparkline (Sprint PP OB.8).
//
// Shows a simple bar chart of hourly token usage (in+out) using Canvas.
// No external chart library — keeps the bundle small.

import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';

final _rollupsProvider =
    FutureProvider.autoDispose<List<Map<String, dynamic>>>((ref) async {
  final client = ref.read(daemonProvider.notifier).client;
  final since = DateTime.now()
          .subtract(const Duration(days: 7))
          .millisecondsSinceEpoch ~/
      1000;
  final result = await client.call('metrics.rollups', {'since': since});
  final list = result['rollups'] as List<dynamic>? ?? [];
  return list.cast<Map<String, dynamic>>();
});

class TokenBurnChart extends ConsumerWidget {
  const TokenBurnChart({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final data = ref.watch(_rollupsProvider);

    return Container(
      height: 120,
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
            'Token Burn (7d)',
            style: TextStyle(fontSize: 11, color: Colors.white54),
          ),
          const SizedBox(height: 6),
          Expanded(
            child: data.when(
              loading: () =>
                  const Center(child: CircularProgressIndicator(strokeWidth: 2)),
              error: (_, __) =>
                  const Center(child: Text('—', style: TextStyle(color: Colors.white38))),
              data: (rollups) {
                if (rollups.isEmpty) {
                  return const Center(
                    child: Text(
                      'No data yet',
                      style: TextStyle(color: Colors.white38, fontSize: 12),
                    ),
                  );
                }
                return CustomPaint(
                  painter: _BarChartPainter(rollups),
                  size: Size.infinite,
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}

class _BarChartPainter extends CustomPainter {
  const _BarChartPainter(this.rollups);
  final List<Map<String, dynamic>> rollups;

  @override
  void paint(Canvas canvas, Size size) {
    if (rollups.isEmpty) return;

    final values = rollups.map((r) {
      final tokensIn = (r['tokens_in'] as num?)?.toInt() ?? 0;
      final tokensOut = (r['tokens_out'] as num?)?.toInt() ?? 0;
      return tokensIn + tokensOut;
    }).toList();

    final maxVal = values.reduce(math.max).toDouble();
    if (maxVal == 0) return;

    final barWidth = size.width / values.length - 2;
    final paintIn = Paint()
      ..color = const Color(0xFF2563EB).withValues(alpha: 0.7)
      ..style = PaintingStyle.fill;

    for (var i = 0; i < values.length; i++) {
      final x = i * (barWidth + 2);
      final barH = (values[i] / maxVal) * size.height;
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromLTWH(x, size.height - barH, barWidth, barH),
          const Radius.circular(2),
        ),
        paintIn,
      );
    }
  }

  @override
  bool shouldRepaint(_BarChartPainter old) => old.rollups != rollups;
}
