// memory_page.dart — AI Memory management page.
//
// Sprint OO ME.6: List, add, delete, search memory entries.

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';

// ── Providers ─────────────────────────────────────────────────────────────────

@immutable
class MemoryEntry {
  const MemoryEntry({
    required this.id,
    required this.scope,
    required this.key,
    required this.value,
    required this.weight,
    required this.source,
  });

  factory MemoryEntry.fromJson(Map<String, dynamic> json) => MemoryEntry(
        id: json['id'] as String? ?? '',
        scope: json['scope'] as String? ?? 'global',
        key: json['key'] as String? ?? '',
        value: json['value'] as String? ?? '',
        weight: (json['weight'] as num?)?.toInt() ?? 5,
        source: json['source'] as String? ?? 'user',
      );

  final String id;
  final String scope;
  final String key;
  final String value;
  final int weight;
  final String source;
}

final memoryEntriesProvider =
    FutureProvider.autoDispose<List<MemoryEntry>>((ref) async {
  final client = ref.read(daemonProvider.notifier).client;
  final result = await client.call('memory.list', {
    'scope': 'global',
    'include_global': true,
  });
  final entries = (result['entries'] as List<dynamic>? ?? [])
      .map((e) => MemoryEntry.fromJson(e as Map<String, dynamic>))
      .toList();
  return entries;
});

// ── Page ──────────────────────────────────────────────────────────────────────

class MemoryPage extends ConsumerStatefulWidget {
  const MemoryPage({super.key});

  @override
  ConsumerState<MemoryPage> createState() => _MemoryPageState();
}

class _MemoryPageState extends ConsumerState<MemoryPage> {
  String _search = '';

  @override
  Widget build(BuildContext context) {
    final entries = ref.watch(memoryEntriesProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('AI Memory'),
        actions: [
          IconButton(
            icon: const Icon(Icons.add),
            tooltip: 'Add memory',
            onPressed: () => _showAddDialog(context),
          ),
        ],
      ),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.all(12),
            child: TextField(
              decoration: const InputDecoration(
                hintText: 'Search memory entries…',
                prefixIcon: Icon(Icons.search),
                border: OutlineInputBorder(),
                isDense: true,
              ),
              onChanged: (v) => setState(() => _search = v.toLowerCase()),
            ),
          ),
          Expanded(
            child: entries.when(
              data: (all) {
                final filtered = _search.isEmpty
                    ? all
                    : all
                        .where((e) =>
                            e.key.toLowerCase().contains(_search) ||
                            e.value.toLowerCase().contains(_search))
                        .toList();

                if (filtered.isEmpty) {
                  return const Center(
                    child: Text(
                      'No memory entries.\nTap + to add one.',
                      textAlign: TextAlign.center,
                      style: TextStyle(color: Colors.white54),
                    ),
                  );
                }

                return ListView.builder(
                  itemCount: filtered.length,
                  itemBuilder: (context, i) =>
                      _MemoryEntryTile(entry: filtered[i], onDelete: _delete),
                );
              },
              loading: () => const Center(child: CircularProgressIndicator()),
              error: (e, _) => Center(
                child: Text('Error: $e', style: const TextStyle(color: Colors.red)),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _delete(MemoryEntry entry) async {
    final confirm = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Delete memory entry?'),
        content: Text('Key: ${entry.key}'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('Delete', style: TextStyle(color: Colors.red)),
          ),
        ],
      ),
    );
    if (confirm != true) return;

    final client = ref.read(daemonProvider.notifier).client;
    await client.call('memory.remove', {'id': entry.id});
    ref.invalidate(memoryEntriesProvider);
  }

  Future<void> _showAddDialog(BuildContext context) async {
    final keyCtrl = TextEditingController();
    final valueCtrl = TextEditingController();
    int weight = 5;

    await showDialog<void>(
      context: context,
      builder: (ctx) => StatefulBuilder(
        builder: (ctx, setState) => AlertDialog(
          title: const Text('Add Memory Entry'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                controller: keyCtrl,
                decoration: const InputDecoration(
                  labelText: 'Key (e.g. preferences.language)',
                ),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: valueCtrl,
                decoration: const InputDecoration(labelText: 'Value'),
                maxLines: 3,
              ),
              const SizedBox(height: 12),
              Row(
                children: [
                  const Text('Weight: '),
                  Expanded(
                    child: Slider(
                      value: weight.toDouble(),
                      min: 1,
                      max: 10,
                      divisions: 9,
                      label: weight.toString(),
                      onChanged: (v) => setState(() => weight = v.round()),
                    ),
                  ),
                  Text(weight.toString()),
                ],
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(ctx),
              child: const Text('Cancel'),
            ),
            TextButton(
              onPressed: () async {
                if (keyCtrl.text.isEmpty || valueCtrl.text.isEmpty) return;
                final client = ref.read(daemonProvider.notifier).client;
                await client.call('memory.add', {
                  'scope': 'global',
                  'key': keyCtrl.text.trim(),
                  'value': valueCtrl.text.trim(),
                  'weight': weight,
                  'source': 'user',
                });
                ref.invalidate(memoryEntriesProvider);
                if (ctx.mounted) Navigator.pop(ctx);
              },
              child: const Text('Add'),
            ),
          ],
        ),
      ),
    );
  }
}

// ── Tile ──────────────────────────────────────────────────────────────────────

class _MemoryEntryTile extends StatelessWidget {
  const _MemoryEntryTile({required this.entry, required this.onDelete});
  final MemoryEntry entry;
  final void Function(MemoryEntry) onDelete;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      leading: _WeightBadge(weight: entry.weight),
      title: Text(entry.key, style: const TextStyle(fontFamily: 'monospace')),
      subtitle: Text(
        entry.value,
        maxLines: 2,
        overflow: TextOverflow.ellipsis,
        style: const TextStyle(color: Colors.white60),
      ),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            entry.scope == 'global' ? 'global' : 'project',
            style: const TextStyle(fontSize: 11, color: Colors.white38),
          ),
          const SizedBox(width: 8),
          IconButton(
            icon: const Icon(Icons.delete_outline, size: 18, color: Colors.white38),
            onPressed: () => onDelete(entry),
          ),
        ],
      ),
    );
  }
}

class _WeightBadge extends StatelessWidget {
  const _WeightBadge({required this.weight});
  final int weight;

  Color get _color {
    if (weight >= 8) return Colors.green;
    if (weight >= 5) return Colors.amber;
    return Colors.grey;
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 28,
      height: 28,
      decoration: BoxDecoration(
        color: _color.withValues(alpha: 0.2),
        border: Border.all(color: _color.withValues(alpha: 0.6)),
        borderRadius: BorderRadius.circular(6),
      ),
      child: Center(
        child: Text(
          weight.toString(),
          style: TextStyle(fontSize: 12, color: _color, fontWeight: FontWeight.bold),
        ),
      ),
    );
  }
}
