import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Settings pane for Remote Access — pairing, paired devices, relay status.
class RemoteAccessSettings extends ConsumerWidget {
  const RemoteAccessSettings({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return ListView(
      padding: const EdgeInsets.all(32),
      children: const [
        _Header(
          title: 'Remote Access',
          subtitle: 'Pair devices and manage relay connectivity',
        ),
        SizedBox(height: 24),

        // ── This Machine ────────────────────────────────────────────────────
        _SectionLabel('This Machine'),
        SizedBox(height: 12),
        _PairNewDeviceCard(),
        SizedBox(height: 28),

        // ── Paired Devices ──────────────────────────────────────────────────
        _SectionLabel('Paired Devices'),
        SizedBox(height: 12),
        _PairedDevicesList(),
        SizedBox(height: 28),

        // ── Relay Status ────────────────────────────────────────────────────
        _SectionLabel('Relay Status'),
        SizedBox(height: 12),
        _RelayStatusCard(),
      ],
    );
  }
}

// ── Pair new device card ──────────────────────────────────────────────────────

class _PairNewDeviceCard extends ConsumerStatefulWidget {
  const _PairNewDeviceCard();

  @override
  ConsumerState<_PairNewDeviceCard> createState() => _PairNewDeviceCardState();
}

class _PairNewDeviceCardState extends ConsumerState<_PairNewDeviceCard> {
  bool _expanded = false;
  Timer? _countdownTimer;
  int _secondsLeft = 0;

  @override
  void dispose() {
    _countdownTimer?.cancel();
    super.dispose();
  }

  void _startCountdown(int seconds) {
    _countdownTimer?.cancel();
    _secondsLeft = seconds;
    _countdownTimer = Timer.periodic(const Duration(seconds: 1), (t) {
      if (!mounted) {
        t.cancel();
        return;
      }
      setState(() => _secondsLeft--);
      if (_secondsLeft <= 0) {
        t.cancel();
        // Regenerate automatically when PIN expires
        ref.read(pairInfoProvider.notifier).regenerate();
      }
    });
  }

  void _expand() {
    setState(() => _expanded = true);
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: _expanded ? _buildExpanded() : _buildCollapsed(),
    );
  }

  Widget _buildCollapsed() {
    return InkWell(
      onTap: _expand,
      borderRadius: BorderRadius.circular(8),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
        child: Row(
          children: [
            Container(
              width: 36,
              height: 36,
              decoration: BoxDecoration(
                color: ClawdTheme.claw.withValues(alpha: 0.12),
                borderRadius: BorderRadius.circular(8),
              ),
              child: const Icon(Icons.add_link, size: 18, color: ClawdTheme.clawLight),
            ),
            const SizedBox(width: 12),
            const Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Pair a New Device',
                    style: TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                      color: Colors.white,
                    ),
                  ),
                  SizedBox(height: 2),
                  Text(
                    'Generate a PIN to connect your phone or another computer',
                    style: TextStyle(fontSize: 11, color: Colors.white38),
                  ),
                ],
              ),
            ),
            const Icon(Icons.chevron_right, size: 18, color: Colors.white38),
          ],
        ),
      ),
    );
  }

  Widget _buildExpanded() {
    final pairAsync = ref.watch(pairInfoProvider);

    return Padding(
      padding: const EdgeInsets.all(20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Text(
                'Pair a New Device',
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: Colors.white,
                ),
              ),
              const Spacer(),
              IconButton(
                icon: const Icon(Icons.close, size: 16, color: Colors.white38),
                tooltip: 'Collapse',
                constraints: const BoxConstraints(),
                padding: EdgeInsets.zero,
                onPressed: () {
                  _countdownTimer?.cancel();
                  setState(() => _expanded = false);
                },
              ),
            ],
          ),
          const SizedBox(height: 4),
          const Text(
            'Open ClawDE on your device and enter this PIN, or scan the QR code.',
            style: TextStyle(fontSize: 11, color: Colors.white38),
          ),
          const SizedBox(height: 20),
          pairAsync.when(
            loading: () => const Center(
              child: Padding(
                padding: EdgeInsets.all(24),
                child: CircularProgressIndicator(),
              ),
            ),
            error: (e, _) => Column(
              children: [
                Text(
                  'Failed to get pairing PIN: $e',
                  style: const TextStyle(fontSize: 12, color: ClawdTheme.error),
                ),
                const SizedBox(height: 12),
                FilledButton.icon(
                  onPressed: () => ref.read(pairInfoProvider.notifier).regenerate(),
                  icon: const Icon(Icons.refresh, size: 14),
                  label: const Text('Retry'),
                  style: FilledButton.styleFrom(backgroundColor: ClawdTheme.claw),
                ),
              ],
            ),
            data: (info) {
              // Start or reset the countdown when we get fresh info.
              WidgetsBinding.instance.addPostFrameCallback((_) {
                if (mounted && (_secondsLeft == 0 || _secondsLeft > info.expiresInSeconds)) {
                  _startCountdown(info.expiresInSeconds);
                }
              });

              final pairingUri = 'clawd://pair?pin=${info.pin}'
                  '&daemon=${info.daemonId}'
                  '&relay=${Uri.encodeComponent(info.relayUrl)}'
                  '&host=${Uri.encodeComponent(info.hostName)}';

              return Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  // QR code
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: Colors.white,
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: QrImageView(
                      data: pairingUri,
                      version: QrVersions.auto,
                      size: 140,
                    ),
                  ),
                  const SizedBox(width: 20),

                  // PIN + metadata
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text(
                          'Pairing PIN',
                          style: TextStyle(fontSize: 11, color: Colors.white38),
                        ),
                        const SizedBox(height: 6),
                        // Large PIN display
                        Row(
                          children: [
                            ...info.pin.split('').asMap().entries.map((e) {
                              final addSpace = e.key == 3;
                              return Row(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  if (addSpace) const SizedBox(width: 8),
                                  Container(
                                    width: 32,
                                    height: 40,
                                    margin: const EdgeInsets.only(right: 4),
                                    decoration: BoxDecoration(
                                      color: ClawdTheme.surface,
                                      borderRadius: BorderRadius.circular(6),
                                      border: Border.all(color: ClawdTheme.claw.withValues(alpha: 0.4)),
                                    ),
                                    alignment: Alignment.center,
                                    child: Text(
                                      e.value,
                                      style: const TextStyle(
                                        fontSize: 20,
                                        fontWeight: FontWeight.w700,
                                        color: ClawdTheme.clawLight,
                                        fontFamily: 'monospace',
                                      ),
                                    ),
                                  ),
                                ],
                              );
                            }),
                          ],
                        ),
                        const SizedBox(height: 10),

                        // Countdown
                        Row(
                          children: [
                            Icon(
                              Icons.timer_outlined,
                              size: 12,
                              color: _secondsLeft < 30 ? ClawdTheme.warning : Colors.white38,
                            ),
                            const SizedBox(width: 4),
                            Text(
                              'Expires in ${_secondsLeft}s',
                              style: TextStyle(
                                fontSize: 11,
                                color: _secondsLeft < 30 ? ClawdTheme.warning : Colors.white38,
                              ),
                            ),
                          ],
                        ),
                        const SizedBox(height: 4),
                        Text(
                          'Host: ${info.hostName}',
                          style: const TextStyle(fontSize: 11, color: Colors.white38),
                          overflow: TextOverflow.ellipsis,
                        ),
                        const SizedBox(height: 12),

                        // Actions
                        Row(
                          children: [
                            OutlinedButton.icon(
                              onPressed: () => Clipboard.setData(ClipboardData(text: info.pin)),
                              icon: const Icon(Icons.copy, size: 12),
                              label: const Text('Copy PIN', style: TextStyle(fontSize: 12)),
                              style: OutlinedButton.styleFrom(
                                foregroundColor: Colors.white60,
                                side: const BorderSide(color: ClawdTheme.surfaceBorder),
                                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                              ),
                            ),
                            const SizedBox(width: 8),
                            OutlinedButton.icon(
                              onPressed: () => ref.read(pairInfoProvider.notifier).regenerate(),
                              icon: const Icon(Icons.refresh, size: 12),
                              label: const Text('New PIN', style: TextStyle(fontSize: 12)),
                              style: OutlinedButton.styleFrom(
                                foregroundColor: Colors.white60,
                                side: const BorderSide(color: ClawdTheme.surfaceBorder),
                                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                              ),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                ],
              );
            },
          ),
        ],
      ),
    );
  }
}

// ── Paired devices list ───────────────────────────────────────────────────────

class _PairedDevicesList extends ConsumerWidget {
  const _PairedDevicesList();

  String _relativeTime(DateTime? dt) {
    if (dt == null) return 'Never';
    final diff = DateTime.now().difference(dt);
    if (diff.inSeconds < 60) return 'Just now';
    if (diff.inMinutes < 60) return '${diff.inMinutes}m ago';
    if (diff.inHours < 24) return '${diff.inHours}h ago';
    return '${diff.inDays}d ago';
  }

  IconData _platformIcon(DevicePlatform platform) => switch (platform.iconName) {
        'smartphone' => Icons.smartphone,
        'laptop_mac' => Icons.laptop_mac,
        'laptop_windows' => Icons.laptop_windows,
        _ => Icons.computer,
      };

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final devicesAsync = ref.watch(pairedDevicesProvider);

    return devicesAsync.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => Text(
        'Failed to load devices: $e',
        style: const TextStyle(fontSize: 12, color: ClawdTheme.error),
      ),
      data: (devices) {
        final active = devices.where((d) => !d.revoked).toList();

        if (active.isEmpty) {
          return Container(
            padding: const EdgeInsets.symmetric(vertical: 24),
            decoration: BoxDecoration(
              color: ClawdTheme.surfaceElevated,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: ClawdTheme.surfaceBorder),
            ),
            child: const Center(
              child: Column(
                children: [
                  Icon(Icons.devices, size: 32, color: Colors.white24),
                  SizedBox(height: 8),
                  Text(
                    'No devices paired yet',
                    style: TextStyle(fontSize: 13, color: Colors.white54),
                  ),
                  SizedBox(height: 4),
                  Text(
                    'Use the pairing panel above to connect a device',
                    style: TextStyle(fontSize: 11, color: Colors.white24),
                  ),
                ],
              ),
            ),
          );
        }

        return Container(
          decoration: BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: ClawdTheme.surfaceBorder),
          ),
          child: ListView.separated(
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            itemCount: active.length,
            separatorBuilder: (_, __) => const Divider(height: 1),
            itemBuilder: (_, i) {
              final device = active[i];
              return _DeviceRow(
                device: device,
                platformIcon: _platformIcon(device.platform),
                lastSeen: _relativeTime(device.lastSeenAt),
                onRevoke: () async {
                  final confirm = await showDialog<bool>(
                    context: context,
                    builder: (_) => AlertDialog(
                      backgroundColor: ClawdTheme.surfaceElevated,
                      title: const Text('Revoke Device', style: TextStyle(color: Colors.white)),
                      content: Text(
                        'Remove "${device.name}" from paired devices? It will no longer be able to connect.',
                        style: const TextStyle(fontSize: 13, color: Colors.white70),
                      ),
                      actions: [
                        TextButton(
                          onPressed: () => Navigator.of(context).pop(false),
                          child: const Text('Cancel'),
                        ),
                        FilledButton(
                          onPressed: () => Navigator.of(context).pop(true),
                          style: FilledButton.styleFrom(backgroundColor: ClawdTheme.error),
                          child: const Text('Revoke'),
                        ),
                      ],
                    ),
                  );
                  if (confirm == true) {
                    await ref.read(pairedDevicesProvider.notifier).revoke(device.id);
                  }
                },
              );
            },
          ),
        );
      },
    );
  }
}

class _DeviceRow extends StatelessWidget {
  const _DeviceRow({
    required this.device,
    required this.platformIcon,
    required this.lastSeen,
    required this.onRevoke,
  });

  final PairedDevice device;
  final IconData platformIcon;
  final String lastSeen;
  final VoidCallback onRevoke;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
      child: Row(
        children: [
          Icon(platformIcon, size: 20, color: Colors.white54),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  device.name,
                  style: const TextStyle(fontSize: 13, color: Colors.white, fontWeight: FontWeight.w500),
                ),
                const SizedBox(height: 2),
                Text(
                  '${device.platform.displayName} · Last seen $lastSeen',
                  style: const TextStyle(fontSize: 11, color: Colors.white38),
                ),
              ],
            ),
          ),
          TextButton.icon(
            onPressed: onRevoke,
            icon: const Icon(Icons.link_off, size: 13),
            label: const Text('Revoke', style: TextStyle(fontSize: 12)),
            style: TextButton.styleFrom(
              foregroundColor: ClawdTheme.error,
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            ),
          ),
        ],
      ),
    );
  }
}

// ── Relay status card ─────────────────────────────────────────────────────────

class _RelayStatusCard extends ConsumerWidget {
  const _RelayStatusCard();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final mode = ref.watch(connectionModeProvider);

    final (label, color, icon, description) = switch (mode) {
      ConnectionMode.local => (
          'Local',
          ClawdTheme.success,
          Icons.computer,
          'Connected to a local daemon on this machine.',
        ),
      ConnectionMode.lan => (
          'LAN',
          ClawdTheme.info,
          Icons.lan,
          'Connected to a daemon on the local network.',
        ),
      ConnectionMode.relay => (
          'Relay',
          ClawdTheme.warning,
          Icons.cloud_queue,
          'Connected via relay server (internet). Requires Personal Remote or Cloud tier.',
        ),
      ConnectionMode.reconnecting => (
          'Reconnecting',
          Colors.orange,
          Icons.sync,
          'Attempting to reconnect to the daemon...',
        ),
      ConnectionMode.offline => (
          'Offline',
          ClawdTheme.error,
          Icons.cloud_off,
          'No daemon connection. Check that clawd is running.',
        ),
    };

    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                decoration: BoxDecoration(
                  color: color.withValues(alpha: 0.12),
                  borderRadius: BorderRadius.circular(20),
                  border: Border.all(color: color.withValues(alpha: 0.4)),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(icon, size: 12, color: color),
                    const SizedBox(width: 5),
                    Text(
                      label,
                      style: TextStyle(
                        fontSize: 12,
                        fontWeight: FontWeight.w600,
                        color: color,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 10),
          Text(
            description,
            style: const TextStyle(fontSize: 12, color: Colors.white54),
          ),
          if (mode == ConnectionMode.offline || mode == ConnectionMode.reconnecting) ...[
            const SizedBox(height: 12),
            const _UpgradeHint(),
          ],
          if (mode != ConnectionMode.relay && mode != ConnectionMode.offline &&
              mode != ConnectionMode.reconnecting) ...[
            const SizedBox(height: 12),
            const _RelayUpgradePrompt(),
          ],
        ],
      ),
    );
  }
}

class _UpgradeHint extends StatelessWidget {
  const _UpgradeHint();

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: ClawdTheme.claw.withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: ClawdTheme.claw.withValues(alpha: 0.2)),
      ),
      child: const Row(
        children: [
          Icon(Icons.rocket_launch_outlined, size: 14, color: ClawdTheme.clawLight),
          SizedBox(width: 8),
          Expanded(
            child: Text(
              r'Upgrade to Personal Remote ($9.99/yr) to connect from anywhere via relay.',
              style: TextStyle(fontSize: 12, color: Colors.white70),
            ),
          ),
        ],
      ),
    );
  }
}

class _RelayUpgradePrompt extends StatelessWidget {
  const _RelayUpgradePrompt();

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: const Row(
        children: [
          Icon(Icons.lock_outline, size: 14, color: Colors.white38),
          SizedBox(width: 8),
          Expanded(
            child: Text(
              r'Relay access requires Personal Remote ($9.99/yr). Currently on LAN/local only.',
              style: TextStyle(fontSize: 12, color: Colors.white54),
            ),
          ),
        ],
      ),
    );
  }
}

// ── Shared helpers ────────────────────────────────────────────────────────────

class _Header extends StatelessWidget {
  const _Header({required this.title, required this.subtitle});
  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title,
          style: const TextStyle(
            fontSize: 18,
            fontWeight: FontWeight.w700,
            color: Colors.white,
          ),
        ),
        const SizedBox(height: 4),
        Text(subtitle, style: const TextStyle(fontSize: 12, color: Colors.white38)),
        const SizedBox(height: 8),
        const Divider(),
      ],
    );
  }
}

class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.text);
  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(
      text,
      style: const TextStyle(
        fontSize: 13,
        fontWeight: FontWeight.w600,
        color: Colors.white70,
      ),
    );
  }
}
