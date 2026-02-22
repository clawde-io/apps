import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde_mobile/features/hosts/host_provider.dart';
import 'package:clawde_mobile/features/hosts/add_host_sheet.dart';

class HostsScreen extends ConsumerWidget {
  const HostsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final hostsAsync = ref.watch(hostListProvider);
    final activeHostId = ref.watch(activeHostIdProvider);
    final daemonState = ref.watch(daemonProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Hosts'),
        actions: [
          IconButton(
            icon: const Icon(Icons.add),
            tooltip: 'Add host',
            onPressed: () => _showAddHostSheet(context),
          ),
        ],
      ),
      body: hostsAsync.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => ErrorState(
          icon: Icons.wifi_off,
          title: 'Failed to load hosts',
          description: e.toString(),
        ),
        data: (hosts) {
          if (hosts.isEmpty) {
            return const Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(Icons.wifi_off, size: 48, color: Colors.white38),
                  SizedBox(height: 16),
                  Text(
                    'No hosts saved',
                    style: TextStyle(
                        fontSize: 18, fontWeight: FontWeight.w600),
                  ),
                  SizedBox(height: 8),
                  Text(
                    'Tap + to add a daemon host',
                    style: TextStyle(color: Colors.white54),
                  ),
                ],
              ),
            );
          }

          return ListView.separated(
            padding: const EdgeInsets.symmetric(vertical: 8),
            itemCount: hosts.length,
            separatorBuilder: (_, __) =>
                const Divider(height: 1, indent: 16),
            itemBuilder: (context, i) {
              final host = hosts[i];
              final isActive = host.id == activeHostId;
              final isConnected =
                  isActive && daemonState.isConnected;

              return ListTile(
                leading: Icon(
                  Icons.circle,
                  size: 12,
                  color: isConnected ? Colors.green : Colors.white24,
                ),
                title: Text(host.name),
                subtitle: Text(
                  host.url,
                  style: const TextStyle(fontSize: 12),
                ),
                trailing: isActive
                    ? const Chip(
                        label: Text('Active'),
                        backgroundColor: Color(0x2566BB6A),
                        labelStyle:
                            TextStyle(color: Colors.green, fontSize: 11),
                        padding: EdgeInsets.zero,
                      )
                    : null,
                onTap: () async {
                  await switchHost(ref, host);
                  if (context.mounted) {
                    ScaffoldMessenger.of(context).showSnackBar(
                      SnackBar(
                        content: Text('Switching to ${host.name}…'),
                        backgroundColor: ClawdTheme.info,
                      ),
                    );
                  }
                },
                onLongPress: () => _confirmDelete(context, ref, host),
              );
            },
          );
        },
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: () => _showAddHostSheet(context),
        backgroundColor: ClawdTheme.claw,
        child: const Icon(Icons.add, color: Colors.white),
      ),
    );
  }

  void _showAddHostSheet(BuildContext context) {
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      builder: (_) => const AddHostSheet(),
    );
  }

  Future<void> _confirmDelete(
      BuildContext context, WidgetRef ref, DaemonHost host) async {
    final ok = await showDialog<bool>(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('Remove host?'),
        content: Text('Remove "${host.name}" from saved hosts?'),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context, false),
              child: const Text('Cancel')),
          TextButton(
              onPressed: () => Navigator.pop(context, true),
              child:
                  const Text('Remove', style: TextStyle(color: Colors.red))),
        ],
      ),
    );
    if (ok == true) {
      await ref.read(hostListProvider.notifier).remove(host.id);
    }
  }
}
