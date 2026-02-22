import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import '../theme/clawd_theme.dart';

/// A compact pill showing the daemon connection status.
/// Tapping while disconnected triggers a reconnect attempt.
class ConnectionStatusIndicator extends ConsumerWidget {
  const ConnectionStatusIndicator({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final daemon = ref.watch(daemonProvider);

    final (label, color, icon) = switch (daemon.status) {
      DaemonStatus.connected => ('Connected', ClawdTheme.success, Icons.circle),
      DaemonStatus.connecting =>
        ('Connecting…', ClawdTheme.warning, Icons.sync),
      DaemonStatus.error => ('Error', ClawdTheme.error, Icons.error_outline),
      DaemonStatus.disconnected => ('Offline', Colors.grey, Icons.circle_outlined),
    };

    return GestureDetector(
      onTap: daemon.status == DaemonStatus.disconnected ||
              daemon.status == DaemonStatus.error
          ? () => ref.read(daemonProvider.notifier).reconnect()
          : null,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        decoration: BoxDecoration(
          color: color.withValues(alpha:0.12),
          borderRadius: BorderRadius.circular(12),
          border: Border.all(color: color.withValues(alpha:0.4)),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 8, color: color),
            const SizedBox(width: 5),
            Text(
              label,
              style: TextStyle(
                fontSize: 11,
                color: color,
                fontWeight: FontWeight.w500,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
