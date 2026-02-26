// SPDX-License-Identifier: MIT
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// RTT connection quality dot shown in the status bar.
///
/// Color coding:
///   green  (<50 ms)  — excellent
///   amber  (<150 ms) — good
///   red    (≥150 ms or degraded) — poor
///
/// Hidden when RTT is 0 (no measurement yet).
/// Tap → shows connectivity detail sheet.
class ConnectionIndicator extends ConsumerWidget {
  const ConnectionIndicator({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final rtt = ref.watch(connectionRttProvider);
    final degraded = ref.watch(connectionDegradedProvider);
    final connectivityAsync = ref.watch(connectivityProvider);

    // Hide when no RTT data yet
    if (rtt == 0 && !degraded) return const SizedBox.shrink();

    final color = _dotColor(rtt, degraded);

    return Tooltip(
      message: degraded
          ? 'Connection degraded  ·  ${rtt}ms'
          : 'RTT: ${rtt}ms',
      child: InkWell(
        onTap: () => _showSheet(
            context, connectivityAsync.valueOrNull ?? const ConnectivityState()),
        borderRadius: BorderRadius.circular(6),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 6),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              _Dot(color: color, pulsing: degraded),
              if (rtt > 0) ...[
                const SizedBox(width: 4),
                Text(
                  '${rtt}ms',
                  style: TextStyle(fontSize: 10, color: color),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Color _dotColor(int rtt, bool degraded) {
    if (degraded || rtt >= 150) return ClawdTheme.error;
    if (rtt >= 50) return ClawdTheme.warning;
    return ClawdTheme.success;
  }

  void _showSheet(BuildContext context, ConnectivityState state) {
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: Colors.transparent,
      builder: (_) => _ConnectivitySheet(state: state),
    );
  }
}

class _Dot extends StatefulWidget {
  const _Dot({required this.color, required this.pulsing});
  final Color color;
  final bool pulsing;

  @override
  State<_Dot> createState() => _DotState();
}

class _DotState extends State<_Dot> with SingleTickerProviderStateMixin {
  AnimationController? _ctrl;

  @override
  void initState() {
    super.initState();
    if (widget.pulsing) {
      _ctrl = AnimationController(
        vsync: this,
        duration: const Duration(milliseconds: 900),
      )..repeat(reverse: true);
    }
  }

  @override
  void didUpdateWidget(_Dot old) {
    super.didUpdateWidget(old);
    if (widget.pulsing && _ctrl == null) {
      _ctrl = AnimationController(
        vsync: this,
        duration: const Duration(milliseconds: 900),
      )..repeat(reverse: true);
    } else if (!widget.pulsing && _ctrl != null) {
      _ctrl!.dispose();
      _ctrl = null;
    }
  }

  @override
  void dispose() {
    _ctrl?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final dot = Container(
      width: 7,
      height: 7,
      decoration: BoxDecoration(shape: BoxShape.circle, color: widget.color),
    );

    if (_ctrl == null) return dot;
    return FadeTransition(opacity: _ctrl!, child: dot);
  }
}

// ─── Detail sheet ─────────────────────────────────────────────────────────────

class _ConnectivitySheet extends StatelessWidget {
  const _ConnectivitySheet({required this.state});
  final ConnectivityState state;

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: const BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const SizedBox(height: 8),
          Container(
            width: 36,
            height: 4,
            decoration: BoxDecoration(
              color: Colors.white24,
              borderRadius: BorderRadius.circular(2),
            ),
          ),
          // Header
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 16, 20, 4),
            child: Row(
              children: [
                const Icon(Icons.wifi, size: 16, color: Colors.white54),
                const SizedBox(width: 8),
                const Text(
                  'Connection Quality',
                  style: TextStyle(
                    fontSize: 15,
                    fontWeight: FontWeight.w700,
                    color: Colors.white,
                  ),
                ),
                const Spacer(),
                _ModeChip(mode: state.mode),
              ],
            ),
          ),
          if (state.degraded)
            const Padding(
              padding: EdgeInsets.fromLTRB(20, 0, 20, 8),
              child: Row(
                children: [
                  Icon(Icons.warning_amber, size: 13, color: ClawdTheme.warning),
                  SizedBox(width: 6),
                  Text(
                    'Connection quality is degraded.',
                    style: TextStyle(fontSize: 12, color: ClawdTheme.warning),
                  ),
                ],
              ),
            ),
          const Divider(height: 1),
          _StatRow(label: 'RTT', value: '${state.rttMs} ms'),
          _StatRow(
            label: 'Packet loss',
            value: '${state.packetLossPct.toStringAsFixed(1)}%',
          ),
          _StatRow(label: 'Mode', value: state.mode),
          if (state.vpnHost != null)
            _StatRow(label: 'VPN host', value: state.vpnHost!),
          _StatRow(
            label: 'LAN peers',
            value: '${state.lanPeers.length}',
          ),
          const SizedBox(height: 24),
        ],
      ),
    );
  }
}

class _ModeChip extends StatelessWidget {
  const _ModeChip({required this.mode});
  final String mode;

  @override
  Widget build(BuildContext context) {
    final color = switch (mode) {
      'direct' => ClawdTheme.success,
      'vpn' => const Color(0xFF3B82F6),
      'offline' => ClawdTheme.error,
      _ => Colors.amber,
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Text(
        mode.toUpperCase(),
        style: TextStyle(
          fontSize: 10,
          fontWeight: FontWeight.w700,
          color: color,
          letterSpacing: 0.4,
        ),
      ),
    );
  }
}

class _StatRow extends StatelessWidget {
  const _StatRow({required this.label, required this.value});
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 9),
      child: Row(
        children: [
          Text(label,
              style: const TextStyle(fontSize: 13, color: Colors.white54)),
          const Spacer(),
          Text(value,
              style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: Colors.white)),
        ],
      ),
    );
  }
}
