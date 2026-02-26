// offline_banner.dart — Offline mode indicator banner (Sprint RR MO.3).
//
// Shows an amber banner at the top of the screen when the daemon is unreachable.
// Wraps any child widget — insert at the top of each scaffold body.

import 'package:clawd_core/clawd_core.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

class OfflineBanner extends ConsumerWidget {
  const OfflineBanner({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final daemonState = ref.watch(daemonProvider);
    final isOffline = daemonState.status == DaemonStatus.disconnected ||
        daemonState.status == DaemonStatus.error;

    return Column(
      children: [
        if (isOffline)
          Container(
            width: double.infinity,
            color: const Color(0xFFF59E0B), // amber-400
            padding: const EdgeInsets.symmetric(vertical: 6, horizontal: 12),
            child: Row(
              children: [
                const Icon(Icons.cloud_off, size: 16, color: Colors.white),
                const SizedBox(width: 8),
                const Expanded(
                  child: Text(
                    'Offline — showing cached data',
                    style: TextStyle(
                      color: Colors.white,
                      fontSize: 13,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                ),
                if (daemonState.errorMessage != null)
                  Text(
                    daemonState.errorMessage!.length > 40
                        ? '${daemonState.errorMessage!.substring(0, 40)}…'
                        : daemonState.errorMessage!,
                    style: const TextStyle(
                      color: Colors.white70,
                      fontSize: 11,
                    ),
                  ),
              ],
            ),
          ),
        Expanded(child: child),
      ],
    );
  }
}
