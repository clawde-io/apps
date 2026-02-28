import 'package:clawd_core/clawd_core.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

// ─── Provider ─────────────────────────────────────────────────────────────────

final _lineageProvider =
    FutureProvider.autoDispose.family<Map<String, dynamic>, String>(
  (ref, taskId) async {
    final client = ref.read(daemonProvider.notifier).client;
    return client.call<Map<String, dynamic>>(
        'task.lineage', {'taskId': taskId});
  },
);

// ─── Widget ───────────────────────────────────────────────────────────────────

/// Collapsible genealogy tree shown in the task detail page.
/// Shows ancestors above and descendants below the current task.
class TaskGenealogyTree extends ConsumerStatefulWidget {
  const TaskGenealogyTree({super.key, required this.taskId});

  final String taskId;

  @override
  ConsumerState<TaskGenealogyTree> createState() => _TaskGenealogyTreeState();
}

class _TaskGenealogyTreeState extends ConsumerState<TaskGenealogyTree> {
  bool _expanded = true;

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(_lineageProvider(widget.taskId));

    return Card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ListTile(
            leading: const Icon(Icons.account_tree_outlined),
            title: const Text('Task Genealogy'),
            trailing: IconButton(
              icon: Icon(
                  _expanded ? Icons.expand_less : Icons.expand_more),
              onPressed: () => setState(() => _expanded = !_expanded),
            ),
          ),
          if (_expanded)
            state.when(
              loading: () => const Padding(
                padding: EdgeInsets.all(16),
                child: LinearProgressIndicator(),
              ),
              error: (e, _) => Padding(
                padding: const EdgeInsets.all(16),
                child: Text('Failed to load lineage: $e',
                    style: const TextStyle(color: Colors.red)),
              ),
              data: (lineage) => _LineageBody(lineage: lineage),
            ),
        ],
      ),
    );
  }
}

class _LineageBody extends StatelessWidget {
  const _LineageBody({required this.lineage});

  final Map<String, dynamic> lineage;

  @override
  Widget build(BuildContext context) {
    final ancestors =
        (lineage['ancestors'] as List<dynamic>?)?.cast<Map<String, dynamic>>() ??
            [];
    final descendants =
        (lineage['descendants'] as List<dynamic>?)?.cast<Map<String, dynamic>>() ??
            [];

    if (ancestors.isEmpty && descendants.isEmpty) {
      return const Padding(
        padding: EdgeInsets.fromLTRB(16, 0, 16, 16),
        child: Text(
          'This task has no parent or child tasks.',
          style: TextStyle(fontStyle: FontStyle.italic),
        ),
      );
    }

    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (ancestors.isNotEmpty) ...[
            _SectionLabel(label: 'Ancestors (${ancestors.length})'),
            ...ancestors.map((a) => _NodeTile(
                  node: a,
                  icon: Icons.arrow_upward,
                  color: Colors.grey,
                )),
          ],
          if (ancestors.isNotEmpty && descendants.isNotEmpty)
            const Divider(height: 24),
          if (descendants.isNotEmpty) ...[
            _SectionLabel(label: 'Descendants (${descendants.length})'),
            ...descendants.map((d) => _NodeTile(
                  node: d,
                  icon: Icons.arrow_downward,
                  color: Theme.of(context).colorScheme.primary,
                  indent: 16,
                )),
          ],
        ],
      ),
    );
  }
}

class _SectionLabel extends StatelessWidget {
  const _SectionLabel({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Text(
        label,
        style: Theme.of(context).textTheme.labelMedium?.copyWith(
              color: Theme.of(context).colorScheme.outline,
            ),
      ),
    );
  }
}

class _NodeTile extends StatelessWidget {
  const _NodeTile({
    required this.node,
    required this.icon,
    required this.color,
    this.indent = 0,
  });

  final Map<String, dynamic> node;
  final IconData icon;
  final Color color;
  final double indent;

  @override
  Widget build(BuildContext context) {
    final title = node['title'] as String? ?? node['taskId'] as String? ?? '—';
    final relationship = node['relationship'] as String? ?? 'spawned_from';

    return Padding(
      padding: EdgeInsets.only(left: indent, bottom: 6),
      child: Row(
        children: [
          Icon(icon, size: 16, color: color),
          const SizedBox(width: 8),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(title,
                    style: Theme.of(context).textTheme.bodyMedium,
                    overflow: TextOverflow.ellipsis),
                Text(
                  relationship.replaceAll('_', ' '),
                  style: Theme.of(context).textTheme.labelSmall?.copyWith(
                        color: Theme.of(context).colorScheme.outline,
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
