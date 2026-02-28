import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/features/repo/repo_context_provider.dart';

final _appVersionProvider = FutureProvider<String>((ref) async {
  final info = await PackageInfo.fromPlatform();
  return 'v${info.version}';
});

/// Aggregate count of pending tool calls across ALL running sessions.
final _totalPendingToolCallsProvider = Provider<int>((ref) {
  final sessions = ref.watch(sessionListProvider).valueOrNull ?? [];
  var total = 0;
  for (final session in sessions) {
    if (session.status == SessionStatus.running) {
      total += ref.watch(pendingToolCallCountProvider(session.id));
    }
  }
  return total;
});

/// Count of sessions in an error state.
final _errorSessionCountProvider = Provider<int>((ref) {
  final sessions = ref.watch(sessionListProvider).valueOrNull ?? [];
  return sessions.where((s) => s.status == SessionStatus.error).length;
});

/// Thin 28px status bar at the bottom of the app window.
/// Shows daemon connection status, active session count, pending tool calls,
/// error count, RAM usage (V02.T11), and app version.
class StatusBar extends ConsumerWidget {
  const StatusBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final daemonState = ref.watch(daemonProvider);
    final sessions = ref.watch(sessionListProvider).valueOrNull ?? [];
    final activeSessions =
        sessions.where((s) => s.status == SessionStatus.running).length;
    final repoAsync = ref.watch(activeRepoStatusProvider);
    final repo = repoAsync.valueOrNull;
    final version = ref.watch(_appVersionProvider).valueOrNull ?? 'v…';
    final pendingToolCalls = ref.watch(_totalPendingToolCallsProvider);
    final errorCount = ref.watch(_errorSessionCountProvider);
    final connectionMode = ref.watch(connectionModeProvider);
    // V02.T11 — resource stats (null when daemon unreachable or still loading)
    final resourceStats = ref.watch(resourceStatsProvider).valueOrNull;

    return Container(
      height: 28,
      decoration: const BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        border: Border(top: BorderSide(color: ClawdTheme.surfaceBorder)),
      ),
      child: Row(
        children: [
          const SizedBox(width: 12),
          // Daemon connection
          _StatusDot(connected: daemonState.isConnected),
          const SizedBox(width: 6),
          Text(
            daemonState.isConnected ? 'Connected' : 'Disconnected',
            style: TextStyle(
              fontSize: 11,
              color: daemonState.isConnected
                  ? ClawdTheme.success
                  : ClawdTheme.error,
            ),
          ),
          // V02.T11 — RAM usage indicator
          if (resourceStats != null) ...[
            const SizedBox(width: 12),
            _RamIndicator(ram: resourceStats.ram, sessions: resourceStats.sessions),
          ],
          // Pending tool calls badge
          if (pendingToolCalls > 0) ...[
            const SizedBox(width: 12),
            _BadgeIndicator(
              count: pendingToolCalls,
              color: ClawdTheme.warning,
              icon: Icons.hourglass_top,
              tooltip: '$pendingToolCalls pending tool call${pendingToolCalls == 1 ? '' : 's'}',
            ),
          ],
          // Error count badge
          if (errorCount > 0) ...[
            const SizedBox(width: 8),
            _BadgeIndicator(
              count: errorCount,
              color: ClawdTheme.error,
              icon: Icons.error_outline,
              tooltip: '$errorCount session${errorCount == 1 ? '' : 's'} with errors',
            ),
          ],
          const Spacer(),
          // Branch + dirty indicator
          if (repo != null && repo.branch != null) ...[
            const Icon(Icons.call_split, size: 12, color: Colors.white38),
            const SizedBox(width: 4),
            Text(
              repo.branch!,
              style: const TextStyle(fontSize: 11, color: Colors.white54),
            ),
            if (repo.files.isNotEmpty) ...[
              const SizedBox(width: 4),
              Container(
                width: 6,
                height: 6,
                decoration: const BoxDecoration(
                  shape: BoxShape.circle,
                  color: Colors.amber,
                ),
              ),
            ],
            if (repo.ahead > 0 || repo.behind > 0) ...[
              const SizedBox(width: 6),
              Text(
                [
                  if (repo.ahead > 0) '↑${repo.ahead}',
                  if (repo.behind > 0) '↓${repo.behind}',
                ].join(' '),
                style: const TextStyle(fontSize: 11, color: Colors.white38),
              ),
            ],
          ],
          const Spacer(),
          // Connection mode chip
          _ConnectionModeChip(mode: connectionMode),
          const SizedBox(width: 12),
          // Session count
          Text(
            '$activeSessions active session${activeSessions == 1 ? '' : 's'}',
            style: TextStyle(
              fontSize: 11,
              color: Colors.white.withValues(alpha: 0.5),
            ),
          ),
          const SizedBox(width: 16),
          // Version
          Text(
            version,
            style: TextStyle(
              fontSize: 11,
              color: Colors.white.withValues(alpha: 0.4),
            ),
          ),
          const SizedBox(width: 12),
        ],
      ),
    );
  }
}

/// V02.T11 — Compact RAM usage pill + session tier counts.
class _RamIndicator extends StatelessWidget {
  const _RamIndicator({required this.ram, required this.sessions});
  final ResourceRam ram;
  final ResourceSessionCounts sessions;

  Color get _color {
    if (ram.usedPercent >= 90) return ClawdTheme.error;
    if (ram.usedPercent >= 75) return ClawdTheme.warning;
    return Colors.white38;
  }

  @override
  Widget build(BuildContext context) {
    final color = _color;
    return Tooltip(
      message: 'RAM: ${ram.usedMb} / ${ram.totalGb} used\n'
          'Daemon: ${ram.daemonMb}\n'
          'Sessions: ${sessions.active} active · ${sessions.warm} warm · ${sessions.cold} cold',
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          // Mini progress bar
          Container(
            width: 28,
            height: 5,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(3),
              color: Colors.white12,
            ),
            child: FractionallySizedBox(
              widthFactor: (ram.usedPercent / 100).clamp(0.0, 1.0),
              alignment: Alignment.centerLeft,
              child: Container(
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(3),
                  color: color,
                ),
              ),
            ),
          ),
          const SizedBox(width: 4),
          Text(
            '${ram.usedPercent}%',
            style: TextStyle(fontSize: 10, color: color),
          ),
        ],
      ),
    );
  }
}

class _StatusDot extends StatelessWidget {
  const _StatusDot({required this.connected});
  final bool connected;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 7,
      height: 7,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: connected ? ClawdTheme.success : ClawdTheme.error,
      ),
    );
  }
}

/// Compact connection-mode pill in the status bar.
/// Shows the current transport mode (Local / LAN / Relay / Reconnecting / Offline).
class _ConnectionModeChip extends StatelessWidget {
  const _ConnectionModeChip({required this.mode});
  final ConnectionMode mode;

  Color get _color => switch (mode) {
        ConnectionMode.local => ClawdTheme.success,
        ConnectionMode.lan => ClawdTheme.info,
        ConnectionMode.relay => ClawdTheme.warning,
        ConnectionMode.reconnecting => Colors.orange,
        ConnectionMode.offline => ClawdTheme.error,
      };

  @override
  Widget build(BuildContext context) {
    final color = _color;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 6,
          height: 6,
          decoration: BoxDecoration(color: color, shape: BoxShape.circle),
        ),
        const SizedBox(width: 4),
        Text(
          mode.displayLabel,
          style: TextStyle(fontSize: 11, color: color),
        ),
      ],
    );
  }
}

/// Compact badge showing an icon and count in the status bar.
/// Used for pending tool calls and error session indicators.
class _BadgeIndicator extends StatelessWidget {
  const _BadgeIndicator({
    required this.count,
    required this.color,
    required this.icon,
    required this.tooltip,
  });

  final int count;
  final Color color;
  final IconData icon;
  final String tooltip;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
        decoration: BoxDecoration(
          color: color.withValues(alpha: 0.15),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: color.withValues(alpha: 0.4)),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 11, color: color),
            const SizedBox(width: 3),
            Text(
              count.toString(),
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w600,
                color: color,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
