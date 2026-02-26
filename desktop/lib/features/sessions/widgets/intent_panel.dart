import 'package:clawd_core/clawd_core.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

// ─── Provider ─────────────────────────────────────────────────────────────────

final _intentProvider =
    FutureProvider.autoDispose.family<Map<String, dynamic>, String>(
  (ref, sessionId) async {
    final client = ref.read(daemonProvider.notifier).client;
    return client.call<Map<String, dynamic>>(
        'session.intentSummary', {'sessionId': sessionId});
  },
);

// ─── Panel ────────────────────────────────────────────────────────────────────

/// Intent vs Execution panel — shown in session detail.
class IntentPanel extends ConsumerWidget {
  const IntentPanel({super.key, required this.sessionId});

  final String sessionId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(_intentProvider(sessionId));

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                const Icon(Icons.compare_arrows),
                const SizedBox(width: 8),
                Text('Intent vs Execution',
                    style: Theme.of(context).textTheme.titleMedium),
              ],
            ),
            const SizedBox(height: 16),
            state.when(
              loading: () => const LinearProgressIndicator(),
              error: (e, _) =>
                  Text('Failed to load: $e', style: const TextStyle(color: Colors.red)),
              data: (summary) => _IntentBody(summary: summary),
            ),
          ],
        ),
      ),
    );
  }
}

class _IntentBody extends StatelessWidget {
  const _IntentBody({required this.summary});

  final Map<String, dynamic> summary;

  @override
  Widget build(BuildContext context) {
    final intent = summary['intent'] as Map<String, dynamic>?;
    final execution = summary['execution'] as Map<String, dynamic>?;
    final divergenceScore = (summary['divergenceScore'] as num?)?.toDouble();

    if (intent == null && execution == null) {
      return const Text(
        'No intent data captured for this session. Intent is extracted from the first user message.',
        style: TextStyle(fontStyle: FontStyle.italic),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // Divergence score.
        if (divergenceScore != null) ...[
          _DivergenceIndicator(score: divergenceScore),
          const SizedBox(height: 16),
        ],
        // Intent.
        if (intent != null) ...[
          _Section(
            title: 'Parsed Intent',
            icon: Icons.track_changes,
            color: Colors.blue,
            children: [
              _JsonKeyValue('Verbs', intent['intent_verbs']),
              _JsonKeyValue('Files', intent['intent_files']),
              _JsonKeyValue('Scope', intent['intent_scope']),
            ],
          ),
          const SizedBox(height: 12),
        ],
        // Execution.
        if (execution != null) ...[
          _Section(
            title: 'Actual Execution',
            icon: Icons.done_all,
            color: Colors.green,
            children: [
              _JsonKeyValue('Files written', execution['files_written']),
              _JsonKeyValue('Tests run', execution['tests_run']),
              _JsonKeyValue('Tasks created', execution['tasks_created']),
            ],
          ),
        ],
      ],
    );
  }
}

class _DivergenceIndicator extends StatelessWidget {
  const _DivergenceIndicator({required this.score});

  final double score;

  Color get _color {
    if (score >= 0.8) return Colors.green;
    if (score >= 0.5) return Colors.orange;
    return Colors.red;
  }

  String get _label {
    if (score >= 0.8) return 'High alignment';
    if (score >= 0.5) return 'Partial alignment';
    return 'Low alignment';
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: _color.withValues(alpha: 0.1),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: _color.withValues(alpha: 0.4)),
      ),
      child: Row(
        children: [
          Icon(Icons.align_horizontal_center, color: _color),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  _label,
                  style: Theme.of(context)
                      .textTheme
                      .titleSmall
                      ?.copyWith(color: _color),
                ),
                LinearProgressIndicator(
                  value: score,
                  color: _color,
                  backgroundColor: _color.withValues(alpha: 0.2),
                ),
              ],
            ),
          ),
          const SizedBox(width: 12),
          Text(
            '${(score * 100).round()}%',
            style: Theme.of(context)
                .textTheme
                .titleMedium
                ?.copyWith(color: _color, fontWeight: FontWeight.bold),
          ),
        ],
      ),
    );
  }
}

class _Section extends StatelessWidget {
  const _Section({
    required this.title,
    required this.icon,
    required this.color,
    required this.children,
  });

  final String title;
  final IconData icon;
  final Color color;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Icon(icon, size: 16, color: color),
            const SizedBox(width: 6),
            Text(
              title,
              style: Theme.of(context)
                  .textTheme
                  .labelMedium
                  ?.copyWith(color: color, fontWeight: FontWeight.w600),
            ),
          ],
        ),
        const SizedBox(height: 8),
        ...children,
      ],
    );
  }
}

class _JsonKeyValue extends StatelessWidget {
  const _JsonKeyValue(this.label, this.value);

  final String label;
  final dynamic value;

  @override
  Widget build(BuildContext context) {
    if (value == null) return const SizedBox.shrink();
    final display = value is List
        ? (value as List).join(', ')
        : value.toString();
    if (display.isEmpty) return const SizedBox.shrink();

    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 100,
            child: Text(
              '$label:',
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                    color: Theme.of(context).colorScheme.outline,
                  ),
            ),
          ),
          Expanded(
            child: Text(display,
                style: Theme.of(context).textTheme.bodySmall),
          ),
        ],
      ),
    );
  }
}
