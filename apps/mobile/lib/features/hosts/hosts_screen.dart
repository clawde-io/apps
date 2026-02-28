import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:multicast_dns/multicast_dns.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde_mobile/features/hosts/host_provider.dart';
import 'package:clawde_mobile/features/hosts/add_host_sheet.dart';

/// Discovered daemon on the local network via mDNS.
class DiscoveredDaemon {
  final String name;
  final String url;
  final String address;
  final int port;

  const DiscoveredDaemon({
    required this.name,
    required this.url,
    required this.address,
    required this.port,
  });
}

/// Provider that performs mDNS/Bonjour discovery for `_clawde._tcp` services.
/// Re-scans when invalidated.
final lanDiscoveryProvider =
    FutureProvider.autoDispose<List<DiscoveredDaemon>>((ref) async {
  final discovered = <DiscoveredDaemon>[];

  MDnsClient? client;
  try {
    client = MDnsClient();
    await client.start();

    // Scan for up to 4 seconds.
    await Future.any([
      Future<void>.delayed(const Duration(seconds: 4)),
      () async {
        await for (final ptr in client!
            .lookup<PtrResourceRecord>(ResourceRecordQuery.serverPointer(
                '_clawde._tcp.local'))) {
          await for (final srv in client.lookup<SrvResourceRecord>(
              ResourceRecordQuery.service(ptr.domainName))) {
            await for (final ip in client.lookup<IPAddressResourceRecord>(
                ResourceRecordQuery.addressIPv4(srv.target))) {
              final url = 'ws://${ip.address.address}:${srv.port}';
              final already = discovered.any((d) => d.url == url);
              if (!already) {
                discovered.add(DiscoveredDaemon(
                  name: ptr.domainName
                      .replaceAll('._clawde._tcp.local', '')
                      .replaceAll('.', ' '),
                  url: url,
                  address: ip.address.address,
                  port: srv.port,
                ));
              }
            }
          }
        }
      }(),
    ]);
  } catch (_) {
    // mDNS failure is non-fatal.
  } finally {
    client?.stop();
  }

  return discovered;
});

class HostsScreen extends ConsumerWidget {
  const HostsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final hostsAsync = ref.watch(hostListProvider);
    final activeHostId = ref.watch(activeHostIdProvider);
    final daemonState = ref.watch(daemonProvider);
    final discoveredAsync = ref.watch(lanDiscoveryProvider);

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
          return CustomScrollView(
            slivers: [
              // ── LAN Discovery section ──────────────────────────────────
              SliverToBoxAdapter(
                child: _LanDiscoverySection(
                  discoveredAsync: discoveredAsync,
                  savedHostUrls:
                      hosts.map((h) => h.url).toSet(),
                  onRefresh: () => ref.invalidate(lanDiscoveryProvider),
                  onAdd: (daemon) => _addDiscoveredHost(context, ref, daemon),
                ),
              ),

              // ── Saved Hosts section ────────────────────────────────────
              if (hosts.isNotEmpty)
                const SliverToBoxAdapter(
                  child: Padding(
                    padding: EdgeInsets.fromLTRB(16, 16, 16, 4),
                    child: Text(
                      'SAVED HOSTS',
                      style: TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.w600,
                        color: Colors.white38,
                        letterSpacing: 0.5,
                      ),
                    ),
                  ),
                ),

              if (hosts.isEmpty)
                const SliverFillRemaining(
                  child: Center(
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
                          'Tap + to add a daemon host\nor discover one on your LAN',
                          textAlign: TextAlign.center,
                          style: TextStyle(color: Colors.white54),
                        ),
                      ],
                    ),
                  ),
                )
              else
                SliverList(
                  delegate: SliverChildBuilderDelegate(
                    (context, i) {
                      final host = hosts[i];
                      final isActive = host.id == activeHostId;
                      final isConnected =
                          isActive && daemonState.isConnected;

                      return Column(
                        children: [
                          ListTile(
                            leading: Icon(
                              Icons.circle,
                              size: 12,
                              color:
                                  isConnected ? Colors.green : Colors.white24,
                            ),
                            title: Row(
                              children: [
                                Flexible(child: Text(host.name)),
                                if (host.isPaired) ...[
                                  const SizedBox(width: 6),
                                  const Icon(
                                    Icons.verified,
                                    size: 14,
                                    color: ClawdTheme.success,
                                  ),
                                ],
                              ],
                            ),
                            subtitle: Text(
                              host.url,
                              style: const TextStyle(fontSize: 12),
                            ),
                            trailing: isActive
                                ? const Chip(
                                    label: Text('Active'),
                                    backgroundColor: Color(0x2566BB6A),
                                    labelStyle: TextStyle(
                                        color: Colors.green, fontSize: 11),
                                    padding: EdgeInsets.zero,
                                  )
                                : null,
                            onTap: () async {
                              await switchHost(ref, host);
                              if (context.mounted) {
                                ScaffoldMessenger.of(context).showSnackBar(
                                  SnackBar(
                                    content:
                                        Text('Switching to ${host.name}...'),
                                    backgroundColor: ClawdTheme.info,
                                  ),
                                );
                              }
                            },
                            onLongPress: () =>
                                _confirmDelete(context, ref, host),
                          ),
                          if (i < hosts.length - 1)
                            const Divider(height: 1, indent: 16),
                        ],
                      );
                    },
                    childCount: hosts.length,
                  ),
                ),
            ],
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

  void _addDiscoveredHost(
    BuildContext context,
    WidgetRef ref,
    DiscoveredDaemon daemon,
  ) {
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      builder: (_) => AddHostSheet(
        prefillUrl: daemon.url,
      ),
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

// ── LAN Discovery section widget ─────────────────────────────────────────────

class _LanDiscoverySection extends StatelessWidget {
  const _LanDiscoverySection({
    required this.discoveredAsync,
    required this.savedHostUrls,
    required this.onRefresh,
    required this.onAdd,
  });

  final AsyncValue<List<DiscoveredDaemon>> discoveredAsync;
  final Set<String> savedHostUrls;
  final VoidCallback onRefresh;
  final void Function(DiscoveredDaemon) onAdd;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 8, 4),
          child: Row(
            children: [
              const Text(
                'LAN DISCOVERY',
                style: TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.w600,
                  color: Colors.white38,
                  letterSpacing: 0.5,
                ),
              ),
              const Spacer(),
              discoveredAsync.isLoading
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : IconButton(
                      icon: const Icon(Icons.refresh, size: 18),
                      tooltip: 'Rescan LAN',
                      onPressed: onRefresh,
                      color: Colors.white54,
                      padding: EdgeInsets.zero,
                      constraints: const BoxConstraints(),
                    ),
            ],
          ),
        ),
        discoveredAsync.when(
          loading: () => const Padding(
            padding: EdgeInsets.symmetric(horizontal: 16, vertical: 12),
            child: Row(
              children: [
                SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                SizedBox(width: 10),
                Text(
                  'Scanning for _clawde._tcp on your network...',
                  style: TextStyle(fontSize: 12, color: Colors.white38),
                ),
              ],
            ),
          ),
          error: (e, _) => Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
            child: Text(
              'Discovery failed: $e',
              style: const TextStyle(fontSize: 12, color: Colors.white38),
            ),
          ),
          data: (daemons) {
            if (daemons.isEmpty) {
              return const Padding(
                padding: EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                child: Text(
                  'No daemons found on your WiFi network.',
                  style: TextStyle(fontSize: 12, color: Colors.white38),
                ),
              );
            }

            return Column(
              children: [
                for (final daemon in daemons)
                  _DiscoveredTile(
                    daemon: daemon,
                    alreadySaved: savedHostUrls.contains(daemon.url),
                    onAdd: () => onAdd(daemon),
                  ),
              ],
            );
          },
        ),
        const Divider(height: 1),
      ],
    );
  }
}

class _DiscoveredTile extends StatelessWidget {
  const _DiscoveredTile({
    required this.daemon,
    required this.alreadySaved,
    required this.onAdd,
  });

  final DiscoveredDaemon daemon;
  final bool alreadySaved;
  final VoidCallback onAdd;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      dense: true,
      leading: Container(
        width: 32,
        height: 32,
        decoration: BoxDecoration(
          color: ClawdTheme.success.withValues(alpha: 0.12),
          borderRadius: BorderRadius.circular(6),
        ),
        child: const Icon(Icons.wifi, size: 16, color: ClawdTheme.success),
      ),
      title: Text(
        daemon.name.isEmpty ? daemon.address : daemon.name,
        style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w500),
      ),
      subtitle: Text(
        '${daemon.address}:${daemon.port}',
        style: const TextStyle(fontSize: 11, color: Colors.white38),
      ),
      trailing: alreadySaved
          ? const Text(
              'Saved',
              style: TextStyle(fontSize: 11, color: Colors.white38),
            )
          : TextButton(
              onPressed: onAdd,
              style: TextButton.styleFrom(
                foregroundColor: ClawdTheme.claw,
                padding:
                    const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
              ),
              child: const Text('Add', style: TextStyle(fontSize: 12)),
            ),
    );
  }
}
