import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_client/clawd_client.dart';
import 'package:clawd_core/clawd_core.dart';

/// Shows a status strip above [child] when the relay is reconnecting or failed.
///
/// Hidden when connected (state == idle or connected).
/// Amber when reconnecting.
/// Red when all retry attempts are exhausted.
class RelayStatusBanner extends ConsumerWidget {
  const RelayStatusBanner({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final relayState = ref.watch(relayStateProvider);
    final connectionMode = ref.watch(connectionModeProvider);

    // Only show the banner when in relay mode and not healthy.
    final showBanner = connectionMode == ConnectionMode.relay &&
        (relayState == RelayConnectionState.reconnecting ||
            relayState == RelayConnectionState.failed ||
            relayState == RelayConnectionState.connecting);

    if (!showBanner) return child;

    final isFailed = relayState == RelayConnectionState.failed;

    return Column(
      children: [
        _RelayBannerStrip(failed: isFailed),
        Expanded(child: child),
      ],
    );
  }
}

class _RelayBannerStrip extends StatelessWidget {
  const _RelayBannerStrip({required this.failed});

  final bool failed;

  @override
  Widget build(BuildContext context) {
    final bg = failed
        ? const Color(0xFF7F1D1D).withValues(alpha: 0.4)   // red-900
        : const Color(0xFF78350F).withValues(alpha: 0.4);  // amber-900
    final fg = failed
        ? const Color(0xFFFCA5A5) // red-300
        : const Color(0xFFFDE68A); // amber-200
    final icon = failed ? Icons.cloud_off : Icons.cloud_sync;
    final label = failed
        ? 'Relay unavailable — check your internet connection.'
        : 'Relay reconnecting…';

    return Container(
      width: double.infinity,
      color: bg,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
      child: Row(
        children: [
          Icon(icon, size: 13, color: fg),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              label,
              style: TextStyle(fontSize: 11, color: fg, fontWeight: FontWeight.w500),
            ),
          ),
          if (!failed)
            SizedBox(
              width: 10,
              height: 10,
              child: CircularProgressIndicator(
                strokeWidth: 1.5,
                color: fg,
              ),
            ),
        ],
      ),
    );
  }
}
