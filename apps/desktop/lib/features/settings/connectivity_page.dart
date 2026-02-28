// SPDX-License-Identifier: MIT
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Connectivity settings page — relay vs VPN host config.
///
/// Shows in Settings → Connectivity. Lets the user configure an explicit
/// VPN/LAN IP for enterprise setups where the daemon is not on localhost.
class ConnectivityPage extends ConsumerStatefulWidget {
  const ConnectivityPage({super.key});

  @override
  ConsumerState<ConnectivityPage> createState() => _ConnectivityPageState();
}

class _ConnectivityPageState extends ConsumerState<ConnectivityPage> {
  late TextEditingController _vpnHostController;
  bool _saving = false;
  String? _saveError;

  @override
  void initState() {
    super.initState();
    final state =
        ref.read(connectivityProvider).valueOrNull ?? const ConnectivityState();
    _vpnHostController = TextEditingController(text: state.vpnHost ?? '');
  }

  @override
  void dispose() {
    _vpnHostController.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final host = _vpnHostController.text.trim();
    setState(() {
      _saving = true;
      _saveError = null;
    });

    try {
      // Build the WS URL from the VPN host. Port defaults to 4300.
      final uri = host.isEmpty
          ? 'ws://127.0.0.1:4300'
          : Uri.tryParse(host)?.hasScheme == true
              ? host
              : 'ws://$host:4300';
      await ref.read(settingsProvider.notifier).setDaemonUrl(uri);

      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(host.isEmpty
                ? 'Switched to local daemon (127.0.0.1:4300)'
                : 'VPN host saved — reconnecting to $host…'),
            backgroundColor: ClawdTheme.surfaceElevated,
          ),
        );
      }
      await ref.read(connectivityProvider.notifier).refresh();
    } catch (e) {
      setState(() => _saveError = e.toString());
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final connectivityAsync = ref.watch(connectivityProvider);
    final state = connectivityAsync.valueOrNull ?? const ConnectivityState();

    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Header
          const Padding(
            padding: EdgeInsets.fromLTRB(24, 20, 24, 4),
            child: Text(
              'Connectivity',
              style: TextStyle(
                fontSize: 15,
                fontWeight: FontWeight.w700,
                color: Colors.white,
              ),
            ),
          ),
          const Padding(
            padding: EdgeInsets.fromLTRB(24, 0, 24, 20),
            child: Text(
              'Configure how the app connects to the clawd daemon.',
              style: TextStyle(fontSize: 12, color: Colors.white38),
            ),
          ),

          // Current status
          _StatusSection(state: state),

          const Divider(color: ClawdTheme.surfaceBorder, height: 1),
          const SizedBox(height: 20),

          // VPN / explicit IP section
          const Padding(
            padding: EdgeInsets.fromLTRB(24, 0, 24, 8),
            child: Text(
              'VPN / Enterprise IP',
              style: TextStyle(
                fontSize: 12,
                fontWeight: FontWeight.w600,
                color: Colors.white54,
                letterSpacing: 0.4,
              ),
            ),
          ),
          const Padding(
            padding: EdgeInsets.fromLTRB(24, 0, 24, 12),
            child: Text(
              'Connect directly to a daemon on your VPN or LAN without going through the relay. '
              'Enter an IP address, hostname, or full ws:// URL.',
              style: TextStyle(fontSize: 12, color: Colors.white38),
            ),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 24),
            child: Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _vpnHostController,
                    style: const TextStyle(fontSize: 13, color: Colors.white),
                    decoration: InputDecoration(
                      hintText: '10.0.1.5  or  ws://10.0.1.5:4300',
                      hintStyle: const TextStyle(
                          fontSize: 13, color: Colors.white24),
                      filled: true,
                      fillColor: ClawdTheme.surfaceElevated,
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide:
                            const BorderSide(color: ClawdTheme.surfaceBorder),
                      ),
                      enabledBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide:
                            const BorderSide(color: ClawdTheme.surfaceBorder),
                      ),
                      focusedBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide:
                            const BorderSide(color: ClawdTheme.claw),
                      ),
                      contentPadding: const EdgeInsets.symmetric(
                          horizontal: 12, vertical: 10),
                    ),
                    onSubmitted: (_) => _save(),
                  ),
                ),
                const SizedBox(width: 10),
                FilledButton(
                  onPressed: _saving ? null : _save,
                  style: FilledButton.styleFrom(
                    backgroundColor: ClawdTheme.claw,
                    foregroundColor: Colors.white,
                    padding: const EdgeInsets.symmetric(
                        horizontal: 16, vertical: 12),
                  ),
                  child: _saving
                      ? const SizedBox(
                          width: 14,
                          height: 14,
                          child: CircularProgressIndicator(
                              strokeWidth: 2, color: Colors.white),
                        )
                      : const Text('Save', style: TextStyle(fontSize: 13)),
                ),
              ],
            ),
          ),
          if (_saveError != null)
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 8, 24, 0),
              child: Text(
                _saveError!,
                style: const TextStyle(fontSize: 12, color: ClawdTheme.error),
              ),
            ),
          const SizedBox(height: 12),
          const Padding(
            padding: EdgeInsets.fromLTRB(24, 0, 24, 4),
            child: Text(
              'Leave blank to use the default local daemon (127.0.0.1:4300).',
              style: TextStyle(fontSize: 11, color: Colors.white24),
            ),
          ),
          const SizedBox(height: 24),
        ],
      ),
    );
  }
}

class _StatusSection extends StatelessWidget {
  const _StatusSection({required this.state});
  final ConnectivityState state;

  Color get _modeColor {
    switch (state.mode) {
      case 'direct':
        return ClawdTheme.success;
      case 'vpn':
        return const Color(0xFF3B82F6); // blue
      case 'offline':
        return ClawdTheme.error;
      default:
        return Colors.amber;
    }
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 0, 24, 20),
      child: Container(
        padding: const EdgeInsets.all(14),
        decoration: BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          borderRadius: BorderRadius.circular(10),
          border: Border.all(color: ClawdTheme.surfaceBorder),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Container(
                  width: 8,
                  height: 8,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    color: _modeColor,
                  ),
                ),
                const SizedBox(width: 8),
                Text(
                  state.mode.toUpperCase(),
                  style: TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w700,
                    color: _modeColor,
                    letterSpacing: 0.5,
                  ),
                ),
                if (state.degraded) ...[
                  const SizedBox(width: 8),
                  const Icon(Icons.warning_amber,
                      size: 13, color: ClawdTheme.warning),
                  const SizedBox(width: 4),
                  const Text(
                    'Degraded',
                    style: TextStyle(
                        fontSize: 11, color: ClawdTheme.warning),
                  ),
                ],
              ],
            ),
            if (state.rttMs > 0) ...[
              const SizedBox(height: 8),
              Row(
                children: [
                  _Stat(label: 'RTT', value: '${state.rttMs} ms'),
                  const SizedBox(width: 20),
                  _Stat(
                    label: 'Loss',
                    value: '${state.packetLossPct.toStringAsFixed(1)}%',
                  ),
                  if (state.vpnHost != null) ...[
                    const SizedBox(width: 20),
                    _Stat(label: 'VPN host', value: state.vpnHost!),
                  ],
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _Stat extends StatelessWidget {
  const _Stat({required this.label, required this.value});
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label,
            style:
                const TextStyle(fontSize: 10, color: Colors.white38)),
        Text(value,
            style: const TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: Colors.white)),
      ],
    );
  }
}
