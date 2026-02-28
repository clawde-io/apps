import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

// ─── Providers ───────────────────────────────────────────────────────────────

final _workflowsProvider = FutureProvider.autoDispose<Map<String, dynamic>>(
  (ref) async {
    final client = ref.read(daemonProvider.notifier).client;
    return client.call<Map<String, dynamic>>('workflow.list', {});
  },
);

// ─── Screen ──────────────────────────────────────────────────────────────────

/// Sprint DD WR.7 — Workflow gallery screen.
///
/// Lists built-in and user-defined workflow recipes. Supports:
/// - Tap "Run" to start a workflow in the current repo
/// - Tap "New" to create a custom workflow
/// - Step progress indicator during execution
class WorkflowsScreen extends ConsumerStatefulWidget {
  const WorkflowsScreen({super.key});

  @override
  ConsumerState<WorkflowsScreen> createState() => _WorkflowsScreenState();
}

class _WorkflowsScreenState extends ConsumerState<WorkflowsScreen> {
  String? _runningRecipeId;

  Future<void> _runWorkflow(String recipeId, String recipeName) async {
    setState(() => _runningRecipeId = recipeId);
    try {
      final client = ref.read(daemonProvider.notifier).client;
      await client.call<Map<String, dynamic>>('workflow.run', {
        'recipeId': recipeId,
        'repoPath': '.',
      });
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Workflow "$recipeName" started')),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Failed to run workflow: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _runningRecipeId = null);
    }
  }

  @override
  Widget build(BuildContext context) {
    final workflowsAsync = ref.watch(_workflowsProvider);

    return Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Text(
                'Workflows',
                style: TextStyle(
                  fontSize: 20,
                  fontWeight: FontWeight.w700,
                  color: Colors.white,
                ),
              ),
              const Spacer(),
              FilledButton.icon(
                onPressed: () => _showCreateSheet(context),
                icon: const Icon(Icons.add, size: 16),
                label: const Text('New Workflow'),
                style: FilledButton.styleFrom(
                  backgroundColor: ClawdTheme.claw,
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          const Text(
            'Automate multi-step AI tasks with reusable workflow recipes.',
            style: TextStyle(fontSize: 13, color: Colors.white54),
          ),
          const SizedBox(height: 24),
          Expanded(
            child: workflowsAsync.when(
              loading: () => const Center(child: CircularProgressIndicator()),
              error: (e, _) => Center(
                child: Text('Failed to load workflows: $e',
                    style: const TextStyle(color: Colors.redAccent)),
              ),
              data: (data) {
                final recipes = (data['recipes'] as List?)
                        ?.cast<Map<String, dynamic>>() ??
                    [];
                if (recipes.isEmpty) {
                  return const Center(
                    child: Text('No workflows found.',
                        style: TextStyle(color: Colors.white38)),
                  );
                }
                return ListView.separated(
                  itemCount: recipes.length,
                  separatorBuilder: (_, __) => const SizedBox(height: 8),
                  itemBuilder: (context, i) {
                    final recipe = recipes[i];
                    return _RecipeCard(
                      recipe: recipe,
                      isRunning: _runningRecipeId == recipe['id'],
                      onRun: () => _runWorkflow(
                        recipe['id'] as String,
                        recipe['name'] as String,
                      ),
                    );
                  },
                );
              },
            ),
          ),
        ],
      ),
    );
  }

  void _showCreateSheet(BuildContext context) {
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      builder: (_) => const _CreateWorkflowSheet(),
    );
  }
}

// ─── Recipe Card ─────────────────────────────────────────────────────────────

class _RecipeCard extends StatelessWidget {
  const _RecipeCard({
    required this.recipe,
    required this.isRunning,
    required this.onRun,
  });

  final Map<String, dynamic> recipe;
  final bool isRunning;
  final VoidCallback onRun;

  @override
  Widget build(BuildContext context) {
    final name = recipe['name'] as String? ?? '';
    final description = recipe['description'] as String? ?? '';
    final isBuiltin = recipe['isBuiltin'] as bool? ?? false;
    final runCount = recipe['runCount'] as int? ?? 0;
    final tags = (recipe['tags'] as List?)?.cast<String>() ?? [];

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              width: 40,
              height: 40,
              decoration: BoxDecoration(
                color: ClawdTheme.claw.withValues(alpha: 0.15),
                borderRadius: BorderRadius.circular(8),
              ),
              child: const Icon(Icons.account_tree_outlined,
                  size: 20, color: ClawdTheme.claw),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Text(
                        name,
                        style: const TextStyle(
                            fontSize: 14, fontWeight: FontWeight.w600),
                      ),
                      if (isBuiltin) ...[
                        const SizedBox(width: 6),
                        Container(
                          padding: const EdgeInsets.symmetric(
                              horizontal: 6, vertical: 2),
                          decoration: BoxDecoration(
                            color: Colors.blue.withValues(alpha: 0.15),
                            borderRadius: BorderRadius.circular(4),
                          ),
                          child: const Text('built-in',
                              style: TextStyle(
                                  fontSize: 10, color: Colors.blue)),
                        ),
                      ],
                    ],
                  ),
                  if (description.isNotEmpty) ...[
                    const SizedBox(height: 4),
                    Text(
                      description,
                      style: const TextStyle(
                          fontSize: 12, color: Colors.white54),
                    ),
                  ],
                  if (tags.isNotEmpty) ...[
                    const SizedBox(height: 6),
                    Wrap(
                      spacing: 4,
                      children: tags
                          .map((t) => Chip(
                                label: Text(t,
                                    style:
                                        const TextStyle(fontSize: 10)),
                                visualDensity: VisualDensity.compact,
                                padding: EdgeInsets.zero,
                              ))
                          .toList(),
                    ),
                  ],
                  const SizedBox(height: 4),
                  Text('$runCount run${runCount == 1 ? '' : 's'}',
                      style: const TextStyle(
                          fontSize: 11, color: Colors.white38)),
                ],
              ),
            ),
            const SizedBox(width: 12),
            isRunning
                ? const SizedBox(
                    width: 24,
                    height: 24,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : FilledButton(
                    onPressed: onRun,
                    style: FilledButton.styleFrom(
                        backgroundColor: ClawdTheme.claw),
                    child: const Text('Run'),
                  ),
          ],
        ),
      ),
    );
  }
}

// ─── Create Workflow Sheet ────────────────────────────────────────────────────

/// Sprint DD WR.8 — Create/edit workflow YAML in a bottom sheet.
class _CreateWorkflowSheet extends ConsumerStatefulWidget {
  const _CreateWorkflowSheet();

  @override
  ConsumerState<_CreateWorkflowSheet> createState() =>
      _CreateWorkflowSheetState();
}

class _CreateWorkflowSheetState extends ConsumerState<_CreateWorkflowSheet> {
  final _nameCtrl = TextEditingController();
  final _descCtrl = TextEditingController();
  final _yamlCtrl = TextEditingController(text: _defaultYaml);
  bool _saving = false;

  static const _defaultYaml = '''steps:
  - prompt: "Describe what you want the AI to do in this step."
    provider: claude
  - prompt: "Follow-up step — inherits context from step 1."
    inherit_from: previous
''';

  @override
  void dispose() {
    _nameCtrl.dispose();
    _descCtrl.dispose();
    _yamlCtrl.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    if (_nameCtrl.text.trim().isEmpty) return;
    setState(() => _saving = true);
    try {
      final client = ref.read(daemonProvider.notifier).client;
      await client.call<Map<String, dynamic>>('workflow.create', {
        'name': _nameCtrl.text.trim(),
        'description': _descCtrl.text.trim(),
        'yaml': _yamlCtrl.text,
      });
      if (mounted) Navigator.pop(context);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Failed to save workflow: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return DraggableScrollableSheet(
      expand: false,
      initialChildSize: 0.85,
      builder: (_, scrollCtrl) => Padding(
        padding: EdgeInsets.only(
          left: 24,
          right: 24,
          top: 24,
          bottom: MediaQuery.of(context).viewInsets.bottom + 24,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('New Workflow',
                style: TextStyle(
                    fontSize: 18,
                    fontWeight: FontWeight.w700,
                    color: Colors.white)),
            const SizedBox(height: 16),
            TextField(
              controller: _nameCtrl,
              decoration: const InputDecoration(
                  labelText: 'Name', border: OutlineInputBorder()),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _descCtrl,
              decoration: const InputDecoration(
                  labelText: 'Description', border: OutlineInputBorder()),
            ),
            const SizedBox(height: 12),
            const Text('Steps (YAML)',
                style: TextStyle(fontSize: 12, color: Colors.white54)),
            const SizedBox(height: 6),
            Expanded(
              child: TextField(
                controller: _yamlCtrl,
                maxLines: null,
                expands: true,
                style: const TextStyle(fontSize: 12, fontFamily: 'monospace'),
                decoration: const InputDecoration(
                  border: OutlineInputBorder(),
                  contentPadding: EdgeInsets.all(12),
                ),
              ),
            ),
            const SizedBox(height: 16),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                TextButton(
                  onPressed: () => Navigator.pop(context),
                  child: const Text('Cancel'),
                ),
                const SizedBox(width: 8),
                FilledButton(
                  onPressed: _saving ? null : _save,
                  style: FilledButton.styleFrom(
                      backgroundColor: ClawdTheme.claw),
                  child: _saving
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child:
                              CircularProgressIndicator(strokeWidth: 2))
                      : const Text('Save Workflow'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
