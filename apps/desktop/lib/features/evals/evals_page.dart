import 'package:clawd_core/clawd_core.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

// ─── Providers ────────────────────────────────────────────────────────────────

final _evalFilesProvider = FutureProvider.autoDispose.family<List<String>, String>(
  (ref, repoPath) async {
    final client = ref.read(daemonProvider.notifier).client;
    final result = await client.call<Map<String, dynamic>>(
        'eval.list', {'repoPath': repoPath});
    final files = result['files'] as List<dynamic>? ?? [];
    return files.cast<String>();
  },
);

final _selectedFileProvider = StateProvider<String>((ref) => 'builtin_evals.yaml');
final _runResultProvider = StateProvider<Map<String, dynamic>?>((ref) => null);
final _runningProvider = StateProvider<bool>((ref) => false);

// ─── Page ─────────────────────────────────────────────────────────────────────

class EvalsPage extends ConsumerWidget {
  const EvalsPage({super.key, required this.repoPath});

  final String repoPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final filesState = ref.watch(_evalFilesProvider(repoPath));
    final selectedFile = ref.watch(_selectedFileProvider);
    final result = ref.watch(_runResultProvider);
    final running = ref.watch(_runningProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Eval Runner'),
        actions: [
          FilledButton.icon(
            icon: running
                ? const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.play_arrow),
            label: Text(running ? 'Running…' : 'Run Evals'),
            onPressed: running
                ? null
                : () => _runEvals(context, ref, selectedFile),
          ),
          const SizedBox(width: 8),
        ],
      ),
      body: Column(
        children: [
          // File selector.
          filesState.when(
            loading: () => const LinearProgressIndicator(),
            error: (_, __) => const SizedBox.shrink(),
            data: (files) => Container(
              padding: const EdgeInsets.all(16),
              child: DropdownButtonFormField<String>(
                initialValue: files.contains(selectedFile) ? selectedFile : files.firstOrNull,
                decoration: const InputDecoration(
                  labelText: 'Eval file',
                  border: OutlineInputBorder(),
                ),
                items: files
                    .map((f) => DropdownMenuItem(value: f, child: Text(f)))
                    .toList(),
                onChanged: (v) {
                  if (v != null) {
                    ref.read(_selectedFileProvider.notifier).state = v;
                    ref.read(_runResultProvider.notifier).state = null;
                  }
                },
              ),
            ),
          ),

          // Results.
          Expanded(
            child: result == null
                ? _PlaceholderView(selectedFile: selectedFile)
                : _ResultsView(result: result),
          ),
        ],
      ),
    );
  }

  Future<void> _runEvals(
    BuildContext context,
    WidgetRef ref,
    String evalFile,
  ) async {
    ref.read(_runningProvider.notifier).state = true;
    ref.read(_runResultProvider.notifier).state = null;
    try {
      final client = ref.read(daemonProvider.notifier).client;
      final r = await client.call<Map<String, dynamic>>('eval.run', {
        'repoPath': repoPath,
        'file': evalFile,
      });
      ref.read(_runResultProvider.notifier).state = r;
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('Eval failed: $e')));
      }
    } finally {
      ref.read(_runningProvider.notifier).state = false;
    }
  }
}

// ─── Results view ─────────────────────────────────────────────────────────────

class _ResultsView extends StatelessWidget {
  const _ResultsView({required this.result});

  final Map<String, dynamic> result;

  @override
  Widget build(BuildContext context) {
    final total = result['total'] as int? ?? 0;
    final passed = result['passed'] as int? ?? 0;
    final failed = result['failed'] as int? ?? 0;
    final score = (result['score'] as num?)?.toDouble() ?? 0.0;
    final results = (result['results'] as List<dynamic>?)?.cast<Map<String, dynamic>>() ?? [];

    return CustomScrollView(
      slivers: [
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Row(
              children: [
                _StatCard(label: 'Total', value: '$total', color: Colors.blue),
                const SizedBox(width: 12),
                _StatCard(
                    label: 'Passed',
                    value: '$passed',
                    color: Colors.green),
                const SizedBox(width: 12),
                _StatCard(
                    label: 'Failed',
                    value: '$failed',
                    color: failed > 0 ? Colors.red : Colors.grey),
                const SizedBox(width: 12),
                _StatCard(
                    label: 'Score',
                    value: '${(score * 100).round()}%',
                    color: score >= 0.8
                        ? Colors.green
                        : score >= 0.5
                            ? Colors.orange
                            : Colors.red),
              ],
            ),
          ),
        ),
        SliverPadding(
          padding: const EdgeInsets.symmetric(horizontal: 16),
          sliver: SliverList(
            delegate: SliverChildBuilderDelegate(
              (ctx, i) => _ResultTile(result: results[i]),
              childCount: results.length,
            ),
          ),
        ),
      ],
    );
  }
}

class _StatCard extends StatelessWidget {
  const _StatCard({
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
      child: Card(
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Column(
            children: [
              Text(value,
                  style: Theme.of(context)
                      .textTheme
                      .headlineSmall
                      ?.copyWith(color: color, fontWeight: FontWeight.bold)),
              Text(label, style: Theme.of(context).textTheme.labelSmall),
            ],
          ),
        ),
      ),
    );
  }
}

class _ResultTile extends StatelessWidget {
  const _ResultTile({required this.result});

  final Map<String, dynamic> result;

  @override
  Widget build(BuildContext context) {
    final passed = result['passed'] as bool? ?? false;
    final name = result['name'] as String? ?? '';
    final reason = result['reason'] as String? ?? '';
    final score = (result['score'] as num?)?.toDouble() ?? 0.0;

    return ListTile(
      leading: Icon(
        passed ? Icons.check_circle : Icons.cancel,
        color: passed ? Colors.green : Colors.red,
      ),
      title: Text(name),
      subtitle: Text(reason),
      trailing: Text(
        '${(score * 100).round()}%',
        style: Theme.of(context).textTheme.labelLarge?.copyWith(
              color: passed ? Colors.green : Colors.red,
            ),
      ),
    );
  }
}

// ─── Placeholder ──────────────────────────────────────────────────────────────

class _PlaceholderView extends StatelessWidget {
  const _PlaceholderView({required this.selectedFile});

  final String selectedFile;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.science_outlined,
              size: 64, color: Theme.of(context).colorScheme.outline),
          const SizedBox(height: 16),
          Text('Ready to run evals',
              style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          Text(
            'File: $selectedFile\nPress "Run Evals" to start.',
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ),
    );
  }
}
