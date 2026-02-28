// SPDX-License-Identifier: MIT
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Direct Mode discovery page — lists LAN peers found via mDNS.
///
/// Shown inside Settings → Connectivity. Users can see which `clawd`
/// instances are visible on the local network and tap "Connect" to
/// switch the client to a direct WebSocket connection (no relay hop).
class DirectModePage extends ConsumerWidget {
  const DirectModePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final connectivityAsync = ref.watch(connectivityProvider);

    return connectivityAsync.when(
      data: (state) => _DirectModeContent(state: state, ref: ref),
      loading: () => const Center(
        child: CircularProgressIndicator(
          color: ClawdTheme.claw,
          strokeWidth: 2,
        ),
      ),
      error: (e, _) => Center(
        child: Text(
          'Failed to load connectivity status: $e',
          style: const TextStyle(color: ClawdTheme.error, fontSize: 13),
        ),
      ),
    );
  }
}

class _DirectModeContent extends ConsumerStatefulWidget {
  const _DirectModeContent({required this.state, required this.ref});
  final ConnectivityState state;
  // ignore: unused_field
  final WidgetRef ref;

  @override
  ConsumerState<_DirectModeContent> createState() => _DirectModeContentState();
}

class _DirectModeContentState extends ConsumerState<_DirectModeContent> {
  bool _scanning = false;

  Future<void> _rescan() async {
    setState(() => _scanning = true);
    await ref.read(connectivityProvider.notifier).refresh();
    if (mounted) setState(() => _scanning = false);
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(connectivityProvider).valueOrNull ?? widget.state;
    final peers = state.lanPeers;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // Header
        Padding(
          padding: const EdgeInsets.fromLTRB(24, 20, 24, 8),
          child: Row(
            children: [
              const Text(
                'LAN Peer Discovery',
                style: TextStyle(
                  fontSize: 15,
                  fontWeight: FontWeight.w700,
                  color: Colors.white,
                ),
              ),
              const Spacer(),
              if (_scanning)
                const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(
                    color: ClawdTheme.claw,
                    strokeWidth: 2,
                  ),
                )
              else
                IconButton(
                  icon: const Icon(Icons.refresh, size: 18, color: Colors.white54),
                  tooltip: 'Rescan',
                  onPressed: _rescan,
                ),
            ],
          ),
        ),
        const Padding(
          padding: EdgeInsets.fromLTRB(24, 0, 24, 16),
          child: Text(
            'ClawDE daemons visible on your local network via mDNS/DNS-SD.',
            style: TextStyle(fontSize: 12, color: Colors.white38),
          ),
        ),

        // Prefer direct toggle
        _SettingTile(
          title: 'Prefer Direct',
          subtitle: 'Try LAN connection first, fall back to relay within 2s',
          value: state.preferDirect,
          onChanged: null, // read-only display — config change requires restart
        ),

        const Divider(color: ClawdTheme.surfaceBorder, height: 1),
        const SizedBox(height: 8),

        // Peer list
        if (peers.isEmpty)
          const Padding(
            padding: EdgeInsets.fromLTRB(24, 16, 24, 16),
            child: Row(
              children: [
                Icon(Icons.wifi_off, size: 16, color: Colors.white24),
                SizedBox(width: 10),
                Text(
                  'No peers found on this network.',
                  style: TextStyle(fontSize: 13, color: Colors.white38),
                ),
              ],
            ),
          )
        else
          ...peers.map((peer) => _PeerTile(peer: peer)),

        const SizedBox(height: 8),
        const Padding(
          padding: EdgeInsets.fromLTRB(24, 0, 24, 4),
          child: Text(
            'Peers are discovered automatically. No configuration needed.',
            style: TextStyle(fontSize: 11, color: Colors.white24),
          ),
        ),
      ],
    );
  }
}

class _PeerTile extends StatelessWidget {
  const _PeerTile({required this.peer});
  final LanPeer peer;

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 24, vertical: 4),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Row(
        children: [
          Container(
            width: 8,
            height: 8,
            decoration: const BoxDecoration(
              shape: BoxShape.circle,
              color: ClawdTheme.success,
            ),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  peer.name,
                  style: const TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    color: Colors.white,
                  ),
                ),
                Text(
                  '${peer.address}:${peer.port}  ·  v${peer.version}',
                  style: const TextStyle(fontSize: 11, color: Colors.white38),
                ),
              ],
            ),
          ),
          Consumer(
            builder: (context, ref, _) => TextButton(
              onPressed: () async {
                await ref
                    .read(settingsProvider.notifier)
                    .setDaemonUrl(peer.wsUrl);
                if (context.mounted) {
                  ScaffoldMessenger.of(context).showSnackBar(
                    SnackBar(
                      content: Text(
                          'Switched to ${peer.address}:${peer.port} — reconnecting…'),
                      backgroundColor: ClawdTheme.surfaceElevated,
                    ),
                  );
                }
              },
              style: TextButton.styleFrom(foregroundColor: ClawdTheme.claw),
              child: const Text('Connect', style: TextStyle(fontSize: 12)),
            ),
          ),
        ],
      ),
    );
  }
}

class _SettingTile extends StatelessWidget {
  const _SettingTile({
    required this.title,
    required this.subtitle,
    required this.value,
    required this.onChanged,
  });
  final String title;
  final String subtitle;
  final bool value;
  final ValueChanged<bool>? onChanged;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 8, 24, 8),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(title,
                    style: const TextStyle(fontSize: 13, color: Colors.white)),
                Text(subtitle,
                    style: const TextStyle(fontSize: 11, color: Colors.white38)),
              ],
            ),
          ),
          Switch(
            value: value,
            onChanged: onChanged,
            activeThumbColor: ClawdTheme.claw,
            activeTrackColor: ClawdTheme.clawDark,
            inactiveThumbColor: Colors.white24,
            inactiveTrackColor: Colors.white12,
          ),
        ],
      ),
    );
  }
}
