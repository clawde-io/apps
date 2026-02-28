import 'package:clawd_core/clawd_core.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

// ─── Provider ─────────────────────────────────────────────────────────────────

final _automationsProvider = FutureProvider.autoDispose<List<Map<String, dynamic>>>(
  (ref) async {
    final client = ref.read(daemonProvider.notifier).client;
    final result = await client.call<Map<String, dynamic>>('automation.list');
    final list = result['automations'] as List<dynamic>? ?? [];
    return list.cast<Map<String, dynamic>>();
  },
);

// ─── Page ─────────────────────────────────────────────────────────────────────

class AutomationsPage extends ConsumerWidget {
  const AutomationsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(_automationsProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Automations'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            tooltip: 'Refresh',
            onPressed: () => ref.invalidate(_automationsProvider),
          ),
          IconButton(
            icon: const Icon(Icons.add),
            tooltip: 'New automation',
            onPressed: () => _showNewAutomationDialog(context, ref),
          ),
        ],
      ),
      body: state.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(Icons.error_outline, size: 48),
              const SizedBox(height: 12),
              Text('Failed to load automations: $e',
                  textAlign: TextAlign.center),
              const SizedBox(height: 12),
              FilledButton(
                onPressed: () => ref.invalidate(_automationsProvider),
                child: const Text('Retry'),
              ),
            ],
          ),
        ),
        data: (automations) => automations.isEmpty
            ? const _EmptyState()
            : ListView.separated(
                padding: const EdgeInsets.all(16),
                itemCount: automations.length,
                separatorBuilder: (_, __) => const SizedBox(height: 8),
                itemBuilder: (_, i) => _AutomationCard(
                  automation: automations[i],
                  onToggle: (enabled) =>
                      _toggle(context, ref, automations[i]['name'] as String, enabled),
                  onTrigger: () =>
                      _trigger(context, ref, automations[i]['name'] as String),
                ),
              ),
      ),
    );
  }

  Future<void> _toggle(
    BuildContext context,
    WidgetRef ref,
    String name,
    bool enabled,
  ) async {
    try {
      final client = ref.read(daemonProvider.notifier).client;
      await client.call<void>('automation.disable', {'name': name, 'enabled': enabled});
      ref.invalidate(_automationsProvider);
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Failed to toggle: $e')),
        );
      }
    }
  }

  Future<void> _trigger(BuildContext context, WidgetRef ref, String name) async {
    try {
      final client = ref.read(daemonProvider.notifier).client;
      await client.call<void>('automation.trigger', {'name': name});
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Triggered: $name')),
        );
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Failed to trigger: $e')),
        );
      }
    }
  }

  void _showNewAutomationDialog(BuildContext context, WidgetRef ref) {
    showDialog(
      context: context,
      builder: (_) => const _NewAutomationDialog(),
    ).then((_) => ref.invalidate(_automationsProvider));
  }
}

// ─── Automation card ──────────────────────────────────────────────────────────

class _AutomationCard extends StatelessWidget {
  const _AutomationCard({
    required this.automation,
    required this.onToggle,
    required this.onTrigger,
  });

  final Map<String, dynamic> automation;
  final void Function(bool) onToggle;
  final VoidCallback onTrigger;

  @override
  Widget build(BuildContext context) {
    final enabled = automation['enabled'] as bool? ?? false;
    final builtin = automation['builtin'] as bool? ?? false;
    final name = automation['name'] as String? ?? '';
    final description = automation['description'] as String? ?? '';
    final trigger = automation['trigger'] as String? ?? '';
    final action = automation['action'] as String? ?? '';

    return Card(
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Text(name,
                              style: Theme.of(context).textTheme.titleMedium),
                          if (builtin) ...[
                            const SizedBox(width: 8),
                            Container(
                              padding: const EdgeInsets.symmetric(
                                  horizontal: 6, vertical: 2),
                              decoration: BoxDecoration(
                                color: Theme.of(context)
                                    .colorScheme
                                    .secondaryContainer,
                                borderRadius: BorderRadius.circular(4),
                              ),
                              child: Text(
                                'built-in',
                                style: Theme.of(context)
                                    .textTheme
                                    .labelSmall
                                    ?.copyWith(
                                      color: Theme.of(context)
                                          .colorScheme
                                          .onSecondaryContainer,
                                    ),
                              ),
                            ),
                          ],
                        ],
                      ),
                      if (description.isNotEmpty) ...[
                        const SizedBox(height: 4),
                        Text(description,
                            style: Theme.of(context).textTheme.bodySmall),
                      ],
                    ],
                  ),
                ),
                Switch(value: enabled, onChanged: onToggle),
              ],
            ),
            const SizedBox(height: 12),
            Row(
              children: [
                _Chip(icon: Icons.bolt, label: trigger),
                const SizedBox(width: 8),
                const Icon(Icons.arrow_forward, size: 14),
                const SizedBox(width: 8),
                _Chip(icon: Icons.play_arrow, label: action),
                const Spacer(),
                TextButton.icon(
                  icon: const Icon(Icons.play_circle_outline, size: 16),
                  label: const Text('Test'),
                  onPressed: onTrigger,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _Chip extends StatelessWidget {
  const _Chip({required this.icon, required this.label});

  final IconData icon;
  final String label;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 12),
          const SizedBox(width: 4),
          Text(label.replaceAll('_', ' '),
              style: Theme.of(context).textTheme.labelSmall),
        ],
      ),
    );
  }
}

// ─── Empty state ──────────────────────────────────────────────────────────────

class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.auto_mode_outlined,
              size: 64,
              color: Theme.of(context).colorScheme.outline),
          const SizedBox(height: 16),
          Text('No automations yet',
              style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          Text(
            'Automations run automatically when events happen.\nAdd one via config or the + button.',
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ),
    );
  }
}

// ─── New automation dialog ────────────────────────────────────────────────────

class _NewAutomationDialog extends StatefulWidget {
  const _NewAutomationDialog();

  @override
  State<_NewAutomationDialog> createState() => _NewAutomationDialogState();
}

class _NewAutomationDialogState extends State<_NewAutomationDialog> {
  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('New Automation'),
      content: const Text(
        'Add automations to your project config:\n\n'
        '.claw/config.toml\n\n'
        '[[automations]]\n'
        'name = "my-automation"\n'
        'trigger = "session_complete"\n'
        'action = "run_tests"\n\n'
        '[automations.action_config]\n'
        'command = "cargo test"',
        style: TextStyle(fontFamily: 'monospace', fontSize: 12),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('Close'),
        ),
      ],
    );
  }
}
