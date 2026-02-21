import 'package:flutter/material.dart';
import 'package:clawd_proto/clawd_proto.dart';
import '../theme/clawd_theme.dart';
import 'provider_badge.dart';

/// A list tile representing a single [Session].
/// Tapping calls [onTap]. Provides visual distinction for active/running sessions.
class SessionListTile extends StatelessWidget {
  const SessionListTile({
    super.key,
    required this.session,
    this.isSelected = false,
    this.onTap,
  });

  final Session session;
  final bool isSelected;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      selected: isSelected,
      selectedTileColor: ClawdTheme.claw.withValues(alpha: 0.12),
      onTap: onTap,
      leading: _StatusDot(status: session.status),
      title: Text(
        session.repoPath.split('/').last,
        style: const TextStyle(fontWeight: FontWeight.w500),
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: Text(
        session.repoPath,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: const TextStyle(fontSize: 11),
      ),
      trailing: ProviderBadge(provider: session.provider),
    );
  }
}

class _StatusDot extends StatelessWidget {
  const _StatusDot({required this.status});
  final SessionStatus status;

  Color get _color => switch (status) {
        SessionStatus.running => ClawdTheme.success,
        SessionStatus.waiting => ClawdTheme.warning,
        SessionStatus.paused => ClawdTheme.info,
        SessionStatus.error => ClawdTheme.error,
        SessionStatus.closed => Colors.grey,
        _ => Colors.grey,
      };

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 8,
      height: 8,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: _color,
        boxShadow: status == SessionStatus.running
            ? [BoxShadow(color: _color.withValues(alpha: 0.5), blurRadius: 4)]
            : null,
      ),
    );
  }
}
