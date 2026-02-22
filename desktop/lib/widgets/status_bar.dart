import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/features/repo/repo_context_provider.dart';

/// Thin 28px status bar at the bottom of the app window.
/// Shows daemon connection status, active session count, and app version.
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
          const Spacer(),
          // Branch + dirty indicator
          if (repo != null && repo.branch != null) ...[
            const Icon(Icons.call_split, size: 12, color: Colors.white38),
            const SizedBox(width: 4),
            Text(
              repo.branch!,
              style: const TextStyle(fontSize: 11, color: Colors.white54),
            ),
            if (repo.isDirty) ...[
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
            if (repo.aheadBy > 0 || repo.behindBy > 0) ...[
              const SizedBox(width: 6),
              Text(
                [
                  if (repo.aheadBy > 0) '↑${repo.aheadBy}',
                  if (repo.behindBy > 0) '↓${repo.behindBy}',
                ].join(' '),
                style: const TextStyle(fontSize: 11, color: Colors.white38),
              ),
            ],
          ],
          const Spacer(),
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
            'v0.1.0',
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
