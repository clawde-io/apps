// cost_panel.dart — Session cost summary panel (Sprint PP OB.7).

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';

final sessionCostProvider =
    FutureProvider.autoDispose.family<Map<String, dynamic>, String>(
  (ref, sessionId) async {
    final client = ref.read(daemonProvider.notifier).client;
    final result = await client.call('metrics.summary', {
      'since': DateTime.now()
          .subtract(const Duration(days: 1))
          .millisecondsSinceEpoch ~/
          1000,
    });
    return result as Map<String, dynamic>;
  },
);

class CostPanel extends ConsumerWidget {
  const CostPanel({super.key, required this.sessionId});
  final String sessionId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final data = ref.watch(sessionCostProvider(sessionId));

    return data.when(
      loading: () => const SizedBox(
        height: 80,
        child: Center(child: CircularProgressIndicator(strokeWidth: 2)),
      ),
      error: (e, _) => const SizedBox.shrink(),
      data: (d) {
        final costUsd = (d['total_cost_usd'] as num?)?.toDouble() ?? 0.0;
        final tokensIn = (d['total_tokens_in'] as num?)?.toInt() ?? 0;
        final tokensOut = (d['total_tokens_out'] as num?)?.toInt() ?? 0;
        final toolCalls = (d['total_tool_calls'] as num?)?.toInt() ?? 0;

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
                'Session Cost',
                style: TextStyle(fontSize: 12, color: Colors.white54),
              ),
              const SizedBox(height: 8),
              Row(
                children: [
                  _StatBox(
                    label: 'Cost',
                    value: '\$${costUsd.toStringAsFixed(4)}',
                    color: const Color(0xFF7C3AED),
                  ),
                  const SizedBox(width: 8),
                  _StatBox(
                    label: 'In',
                    value: _fmtTokens(tokensIn),
                    color: const Color(0xFF2563EB),
                  ),
                  const SizedBox(width: 8),
                  _StatBox(
                    label: 'Out',
                    value: _fmtTokens(tokensOut),
                    color: const Color(0xFF059669),
                  ),
                  const SizedBox(width: 8),
                  _StatBox(
                    label: 'Tools',
                    value: '$toolCalls',
                    color: const Color(0xFFD97706),
                  ),
                ],
              ),
            ],
          ),
        );
      },
    );
  }

  String _fmtTokens(int t) {
    if (t >= 1000000) return '${(t / 1000000).toStringAsFixed(1)}M';
    if (t >= 1000) return '${(t / 1000).toStringAsFixed(1)}k';
    return '$t';
  }
}

class _StatBox extends StatelessWidget {
  const _StatBox({
    required this.label,
    required this.value,
    required this.color,
  });
  final String label;
  final String value;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Expanded(
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        decoration: BoxDecoration(
          color: color.withValues(alpha: 0.08),
          borderRadius: BorderRadius.circular(6),
          border: Border.all(color: color.withValues(alpha: 0.3)),
        ),
        child: Column(
          children: [
            Text(
              value,
              style: TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.bold,
                color: color,
              ),
            ),
            Text(
              label,
              style: const TextStyle(fontSize: 10, color: Colors.white38),
            ),
          ],
        ),
      ),
    );
  }
}
