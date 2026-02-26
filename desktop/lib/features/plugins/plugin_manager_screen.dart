import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';

// ─── Provider ─────────────────────────────────────────────────────────────────

final _pluginListProvider =
    FutureProvider.autoDispose<List<Map<String, dynamic>>>((ref) async {
  final client = ref.read(daemonProvider.notifier).client;
  final result =
      await client.call<Map<String, dynamic>>('plugin.list', {});
  final plugins = result['plugins'] as List<dynamic>? ?? [];
  return plugins.cast<Map<String, dynamic>>();
});

// ─── Screen ───────────────────────────────────────────────────────────────────

/// Sprint FF PL.7 — Plugin Manager page.
///
/// Lists installed plugins with status, enable/disable toggle, and detail view.
class PluginManagerScreen extends ConsumerWidget {
  const PluginManagerScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final pluginsAsync = ref.watch(_pluginListProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Plugins'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: () => ref.invalidate(_pluginListProvider),
          ),
        ],
      ),
      body: pluginsAsync.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(
          child: Text('Failed to load plugins: $e',
              style: const TextStyle(color: Colors.redAccent)),
        ),
        data: (plugins) => plugins.isEmpty
            ? const _EmptyState()
            : _PluginList(plugins: plugins),
      ),
    );
  }
}

// ─── Empty state ──────────────────────────────────────────────────────────────

class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.extension_off_outlined,
              size: 48, color: Colors.white24),
          SizedBox(height: 12),
          Text(
            'No plugins installed.',
            style: TextStyle(color: Colors.white54),
          ),
          SizedBox(height: 6),
          Text(
            'Install plugins with:\nclawd pack install <plugin-name>',
            textAlign: TextAlign.center,
            style: TextStyle(
              fontFamily: 'monospace',
              fontSize: 12,
              color: Colors.white38,
            ),
          ),
        ],
      ),
    );
  }
}

// ─── Plugin list ──────────────────────────────────────────────────────────────

class _PluginList extends ConsumerWidget {
  const _PluginList({required this.plugins});

  final List<Map<String, dynamic>> plugins;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return ListView.builder(
      padding: const EdgeInsets.all(16),
      itemCount: plugins.length,
      itemBuilder: (context, index) => _PluginTile(plugin: plugins[index]),
    );
  }
}

// ─── Plugin tile ──────────────────────────────────────────────────────────────

class _PluginTile extends ConsumerStatefulWidget {
  const _PluginTile({required this.plugin});

  final Map<String, dynamic> plugin;

  @override
  ConsumerState<_PluginTile> createState() => _PluginTileState();
}

class _PluginTileState extends ConsumerState<_PluginTile> {
  bool _toggling = false;

  String get _status => widget.plugin['status'] as String? ?? 'unknown';
  bool get _enabled => _status == 'enabled';
  String get _name => widget.plugin['name'] as String? ?? '?';
  String get _version => widget.plugin['version'] as String? ?? '?';
  String get _runtime => widget.plugin['runtime'] as String? ?? '?';
  bool get _signed => widget.plugin['is_signed'] as bool? ?? false;

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: ExpansionTile(
        leading: Icon(
          Icons.extension,
          color: _enabled ? Colors.green : Colors.white38,
          size: 20,
        ),
        title: Text(
          '$_name@$_version',
          style: const TextStyle(fontSize: 14),
        ),
        subtitle: Text(
          '$_runtime · ${_enabled ? "enabled" : _status}',
          style: TextStyle(
            fontSize: 11,
            color: _enabled ? Colors.white54 : Colors.orange,
          ),
        ),
        trailing: _toggling
            ? const SizedBox(
                width: 20,
                height: 20,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            : Switch(
                value: _enabled,
                onChanged: (v) => _toggle(v),
              ),
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _DetailRow(label: 'Runtime', value: _runtime),
                _DetailRow(label: 'Status', value: _status),
                _DetailRow(
                  label: 'Signature',
                  value: _signed ? 'Verified ✓' : 'Self-signed',
                ),
                if (widget.plugin['path'] != null)
                  _DetailRow(
                    label: 'Path',
                    value: widget.plugin['path'] as String,
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _toggle(bool enable) async {
    setState(() => _toggling = true);
    try {
      final client = ref.read(daemonProvider.notifier).client;
      final method = enable ? 'plugin.enable' : 'plugin.disable';
      await client.call<Map<String, dynamic>>(method, {'name': _name});
      ref.invalidate(_pluginListProvider);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Failed to ${enable ? "enable" : "disable"} plugin: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _toggling = false);
    }
  }
}

class _DetailRow extends StatelessWidget {
  const _DetailRow({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 80,
            child: Text(
              label,
              style: const TextStyle(fontSize: 11, color: Colors.white38),
            ),
          ),
          Expanded(
            child: Text(
              value,
              style: const TextStyle(
                fontSize: 11,
                fontFamily: 'monospace',
                color: Colors.white70,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
