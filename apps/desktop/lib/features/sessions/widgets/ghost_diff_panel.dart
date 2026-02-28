import 'package:clawd_core/clawd_core.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

// ─── Provider ─────────────────────────────────────────────────────────────────

final _ghostDiffProvider =
    FutureProvider.autoDispose.family<Map<String, dynamic>, String>(
  (ref, repoPath) async {
    final client = ref.read(daemonProvider.notifier).client;
    return client.call<Map<String, dynamic>>(
        'ghost_diff.check', {'repoPath': repoPath});
  },
);

// ─── Panel ────────────────────────────────────────────────────────────────────

class GhostDiffPanel extends ConsumerWidget {
  const GhostDiffPanel({super.key, required this.repoPath});

  final String repoPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(_ghostDiffProvider(repoPath));

    return Scaffold(
      appBar: AppBar(
        title: const Text('Ghost Diff'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            tooltip: 'Re-run check',
            onPressed: () => ref.invalidate(_ghostDiffProvider(repoPath)),
          ),
        ],
      ),
      body: state.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('Error: $e')),
        data: (result) {
          final warnings =
              (result['warnings'] as List<dynamic>?)?.cast<Map<String, dynamic>>() ?? [];
          final hasDrift = result['hasDrift'] as bool? ?? false;

          if (!hasDrift) {
            return const _NoDriftView();
          }

          return ListView(
            padding: const EdgeInsets.all(16),
            children: [
              _DriftBanner(count: warnings.length),
              const SizedBox(height: 16),
              ...warnings.map(
                (w) => _DriftCard(warning: w, repoPath: repoPath, ref: ref),
              ),
            ],
          );
        },
      ),
    );
  }
}

// ─── Sub-widgets ──────────────────────────────────────────────────────────────

class _NoDriftView extends StatelessWidget {
  const _NoDriftView();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.check_circle_outline,
              size: 64, color: Colors.green),
          const SizedBox(height: 16),
          Text('No spec drift detected',
              style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          Text(
            'All changed files align with the specs in .claw/specs/',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ),
    );
  }
}

class _DriftBanner extends StatelessWidget {
  const _DriftBanner({required this.count});

  final int count;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: Colors.orange.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: Colors.orange.withValues(alpha: 0.5)),
      ),
      child: Row(
        children: [
          const Icon(Icons.warning_amber_outlined, color: Colors.orange),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              '$count spec violation${count == 1 ? '' : 's'} detected. '
              'Review and accept or revert each divergence.',
              style: const TextStyle(color: Colors.orange),
            ),
          ),
        ],
      ),
    );
  }
}

class _DriftCard extends StatelessWidget {
  const _DriftCard({
    required this.warning,
    required this.repoPath,
    required this.ref,
  });

  final Map<String, dynamic> warning;
  final String repoPath;
  final WidgetRef ref;

  @override
  Widget build(BuildContext context) {
    final file = warning['file'] as String? ?? '';
    final spec = warning['spec'] as String? ?? '';
    final summary = warning['divergenceSummary'] as String? ?? '';
    final severity = warning['severity'] as String? ?? 'medium';

    final severityColor = switch (severity) {
      'high' => Colors.red,
      'medium' => Colors.orange,
      _ => Colors.yellow.shade700,
    };

    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Header row.
            Row(
              children: [
                Icon(Icons.difference_outlined,
                    size: 20, color: severityColor),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(file,
                      style: Theme.of(context).textTheme.titleSmall,
                      overflow: TextOverflow.ellipsis),
                ),
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                  decoration: BoxDecoration(
                    color: severityColor.withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(4),
                  ),
                  child: Text(
                    severity,
                    style: TextStyle(
                        color: severityColor,
                        fontSize: 11,
                        fontWeight: FontWeight.w600),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 8),
            Text('Spec: $spec',
                style: Theme.of(context).textTheme.labelSmall?.copyWith(
                      color: Theme.of(context).colorScheme.outline,
                    )),
            const SizedBox(height: 8),
            Text(summary, style: Theme.of(context).textTheme.bodySmall),
            const SizedBox(height: 12),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                OutlinedButton.icon(
                  icon: const Icon(Icons.undo, size: 16),
                  label: const Text('Revert'),
                  onPressed: () => _showRevertInfo(context),
                ),
                const SizedBox(width: 8),
                FilledButton.icon(
                  icon: const Icon(Icons.check, size: 16),
                  label: const Text('Accept Drift'),
                  onPressed: () => _acceptDrift(context),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  void _showRevertInfo(BuildContext context) {
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('Revert changes'),
        content: Text(
          'To revert changes to "${warning['file']}", run:\n\n'
          'git checkout -- ${warning['file']}',
          style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Close'),
          ),
        ],
      ),
    );
  }

  void _acceptDrift(BuildContext context) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
            'Drift accepted for ${warning['file']}. Update the spec to reflect current behavior.'),
      ),
    );
    ref.invalidate(_ghostDiffProvider(repoPath));
  }
}
