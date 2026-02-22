import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

class SessionHeader extends ConsumerWidget {
  const SessionHeader({super.key, required this.session});

  final Session session;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final repoName = session.repoPath.split('/').last;

    return Container(
      height: 48,
      padding: const EdgeInsets.symmetric(horizontal: 16),
      decoration: const BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        border: Border(bottom: BorderSide(color: ClawdTheme.surfaceBorder)),
      ),
      child: Row(
        children: [
          Text(
            repoName,
            style: const TextStyle(
              fontWeight: FontWeight.w600,
              fontSize: 14,
            ),
          ),
          const SizedBox(width: 10),
          ProviderBadge(provider: session.provider),
          const SizedBox(width: 10),
          _StatusChip(status: session.status),
          const Spacer(),
          if (session.status == SessionStatus.running)
            IconButton(
              icon: const Icon(Icons.pause, size: 18),
              tooltip: 'Pause',
              onPressed: () =>
                  ref.read(sessionListProvider.notifier).pause(session.id),
            ),
          if (session.status == SessionStatus.paused)
            IconButton(
              icon: const Icon(Icons.play_arrow, size: 18),
              tooltip: 'Resume',
              onPressed: () =>
                  ref.read(sessionListProvider.notifier).resume(session.id),
            ),
          IconButton(
            icon: const Icon(Icons.close, size: 18),
            tooltip: 'Close',
            onPressed: () async {
              await ref.read(sessionListProvider.notifier).close(session.id);
              ref.read(activeSessionIdProvider.notifier).state = null;
            },
          ),
        ],
      ),
    );
  }
}

class _StatusChip extends StatelessWidget {
  const _StatusChip({required this.status});
  final SessionStatus status;

  (String, Color) get _label => switch (status) {
        SessionStatus.idle => ('Idle', Colors.grey),
        SessionStatus.running => ('Running', ClawdTheme.success),
        SessionStatus.paused => ('Paused', ClawdTheme.warning),
        SessionStatus.completed => ('Done', ClawdTheme.info),
        SessionStatus.error => ('Error', ClawdTheme.error),
      };

  @override
  Widget build(BuildContext context) {
    final (label, color) = _label;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontSize: 11,
          color: color,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}
