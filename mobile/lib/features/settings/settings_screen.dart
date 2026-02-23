import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:clawd_client/clawd_client.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde_mobile/features/hosts/host_provider.dart';
import 'package:clawde_mobile/features/hosts/qr_scanner_sheet.dart';

final _appVersionProvider = FutureProvider<String>((ref) async {
  final info = await PackageInfo.fromPlatform();
  return 'v${info.version}';
});

class SettingsScreen extends ConsumerWidget {
  const SettingsScreen({super.key});

  static final _repoUrl = Uri.parse('https://github.com/clawde-io/apps');

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final hostsAsync = ref.watch(hostListProvider);
    final activeHostId = ref.watch(activeHostIdProvider);
    final daemonState = ref.watch(daemonProvider);

    // Find the currently active host (may be null if none selected yet).
    final activeHost = hostsAsync.valueOrNull
        ?.where((h) => h.id == activeHostId)
        .firstOrNull;

    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: ListView(
        children: [
          // ── Host / Connection section ────────────────────────────────────
          const Padding(
            padding: EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Text(
              'HOST',
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w600,
                color: Colors.white38,
                letterSpacing: 0.5,
              ),
            ),
          ),
          if (activeHost != null)
            ListTile(
              leading: Icon(
                Icons.circle,
                size: 12,
                color: daemonState.isConnected ? Colors.green : Colors.white24,
              ),
              title: Text(activeHost.name),
              subtitle: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    activeHost.url,
                    style: const TextStyle(fontSize: 11),
                  ),
                  const SizedBox(height: 4),
                  _ConnectionModeChip(ref: ref),
                ],
              ),
              isThreeLine: true,
              trailing: activeHost.isPaired
                  ? Tooltip(
                      message: 'Re-pair via QR code',
                      child: IconButton(
                        icon: const Icon(Icons.qr_code_scanner, size: 20),
                        onPressed: () => _openQrScanner(context),
                      ),
                    )
                  : null,
              onTap: () => ref.read(daemonProvider.notifier).reconnect(),
            )
          else
            ListTile(
              leading: const Icon(Icons.wifi_off, size: 20, color: Colors.white38),
              title: const Text('No host selected'),
              subtitle: const Text(
                'Go to Hosts to add or pair a daemon',
                style: TextStyle(fontSize: 12),
              ),
              trailing: IconButton(
                icon: const Icon(Icons.qr_code_scanner, size: 20),
                tooltip: 'Pair via QR code',
                onPressed: () => _openQrScanner(context),
              ),
            ),
          const Divider(),
          // ── Daemon URL (raw) ─────────────────────────────────────────────
          const Padding(
            padding: EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Text(
              'DAEMON',
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w600,
                color: Colors.white38,
                letterSpacing: 0.5,
              ),
            ),
          ),
          ListTile(
            title: const Text('Connection'),
            subtitle: Text(
              ref.watch(settingsProvider).valueOrNull?.daemonUrl ??
                  'ws://127.0.0.1:4300',
              style: const TextStyle(fontSize: 12),
            ),
            trailing: const ConnectionStatusIndicator(),
            onTap: () => ref.read(daemonProvider.notifier).reconnect(),
          ),
          const Divider(),
          // ── About section ────────────────────────────────────────────────
          const Padding(
            padding: EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Text(
              'ABOUT',
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w600,
                color: Colors.white38,
                letterSpacing: 0.5,
              ),
            ),
          ),
          ListTile(
            title: const Text('ClawDE'),
            subtitle: Text(
              ref.watch(_appVersionProvider).valueOrNull ?? 'v...',
            ),
          ),
          ListTile(
            title: const Text('Source'),
            subtitle: const Text('github.com/clawde-io/apps'),
            trailing: const Icon(Icons.open_in_new, size: 16),
            onTap: () =>
                launchUrl(_repoUrl, mode: LaunchMode.externalApplication),
          ),
        ],
      ),
    );
  }

  void _openQrScanner(BuildContext context) {
    Navigator.of(context).push<void>(
      MaterialPageRoute<void>(
        fullscreenDialog: true,
        builder: (_) => const QrScannerSheet(),
      ),
    );
  }
}

// ─── Connection mode chip ─────────────────────────────────────────────────────

/// Small chip showing whether the active connection is Local, Relay, or Offline.
class _ConnectionModeChip extends StatelessWidget {
  const _ConnectionModeChip({required this.ref});

  final WidgetRef ref;

  @override
  Widget build(BuildContext context) {
    final daemonState = ref.watch(daemonProvider);

    if (!daemonState.isConnected) {
      return _chip(
        label: 'Offline',
        color: Colors.white38,
        icon: Icons.wifi_off,
      );
    }

    // Read the connection mode from the client directly.
    final client = ref.read(daemonProvider.notifier).client;
    final mode = client.connectionMode;

    switch (mode) {
      case ClawdConnectionMode.relay:
        return _chip(
          label: 'Relay',
          color: ClawdTheme.info,
          icon: Icons.cloud_outlined,
        );
      case ClawdConnectionMode.lan:
        return _chip(
          label: 'Local',
          color: ClawdTheme.success,
          icon: Icons.wifi,
        );
      case ClawdConnectionMode.offline:
        return _chip(
          label: 'Offline',
          color: Colors.white38,
          icon: Icons.wifi_off,
        );
    }
  }

  Widget _chip({
    required String label,
    required Color color,
    required IconData icon,
  }) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 11, color: color),
        const SizedBox(width: 4),
        Text(
          label,
          style: TextStyle(fontSize: 11, color: color),
        ),
      ],
    );
  }
}
