import 'dart:developer' as dev;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Wraps [child] and shows a persistent banner at the top when the daemon
/// broadcasts a `daemon.updateAvailable` push event.
///
/// The banner reads: "Update Available (vX.Y.Z) — restart to apply"
/// with a "Restart to Apply" action button and a dismiss button.
///
/// Tapping "Restart to Apply" calls `daemon.applyUpdate` and then hides
/// the banner optimistically (the daemon will restart and disconnect).
class UpdateBanner extends ConsumerStatefulWidget {
  const UpdateBanner({super.key, required this.child});

  final Widget child;

  @override
  ConsumerState<UpdateBanner> createState() => _UpdateBannerState();
}

class _UpdateBannerState extends ConsumerState<UpdateBanner> {
  /// Latest version string received from the daemon, or null when no update
  /// is available or the banner has been dismissed.
  String? _latestVersion;
  bool _applying = false;

  void _dismiss() => setState(() => _latestVersion = null);

  Future<void> _applyUpdate() async {
    setState(() => _applying = true);
    try {
      final client = ref.read(daemonProvider.notifier).client;
      await client.call<Map<String, dynamic>>('daemon.applyUpdate', {});
    } catch (e) {
      dev.log('daemon.applyUpdate failed: $e', name: 'UpdateBanner');
    } finally {
      // Daemon restarts — banner disappears naturally on reconnect.
      if (mounted) setState(() => _applying = false);
    }
    _dismiss();
  }

  @override
  Widget build(BuildContext context) {
    // Listen for the daemon.updateAvailable push event.
    ref.listen<AsyncValue<Map<String, dynamic>>>(
      daemonPushEventsProvider,
      (_, next) {
        next.whenData((event) {
          if (event['method'] == 'daemon.updateAvailable') {
            final params = event['params'] as Map<String, dynamic>?;
            final latest = params?['latest'] as String?;
            if (latest != null && latest != _latestVersion) {
              setState(() => _latestVersion = latest);
            }
          }
        });
      },
    );

    return Column(
      children: [
        if (_latestVersion != null)
          _UpdateBannerStrip(
            version: _latestVersion!,
            applying: _applying,
            onApply: _applyUpdate,
            onDismiss: _dismiss,
          ),
        Expanded(child: widget.child),
      ],
    );
  }
}

// ─── Banner strip ─────────────────────────────────────────────────────────────

class _UpdateBannerStrip extends StatelessWidget {
  const _UpdateBannerStrip({
    required this.version,
    required this.applying,
    required this.onApply,
    required this.onDismiss,
  });

  final String version;
  final bool applying;
  final VoidCallback onApply;
  final VoidCallback onDismiss;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      color: ClawdTheme.claw.withValues(alpha: 0.15),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      child: Row(
        children: [
          const Icon(
            Icons.system_update_outlined,
            size: 14,
            color: ClawdTheme.clawLight,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              'Update Available (v$version) — restart to apply',
              style: const TextStyle(
                fontSize: 12,
                color: ClawdTheme.clawLight,
                fontWeight: FontWeight.w500,
              ),
            ),
          ),
          TextButton(
            onPressed: applying ? null : onApply,
            style: TextButton.styleFrom(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
              minimumSize: Size.zero,
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            ),
            child: applying
                ? const SizedBox(
                    width: 12,
                    height: 12,
                    child: CircularProgressIndicator(
                      strokeWidth: 1.5,
                      color: ClawdTheme.clawLight,
                    ),
                  )
                : const Text(
                    'Restart to Apply',
                    style: TextStyle(
                      fontSize: 11,
                      color: ClawdTheme.clawLight,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
          ),
          const SizedBox(width: 4),
          InkWell(
            onTap: onDismiss,
            borderRadius: BorderRadius.circular(4),
            child: const Padding(
              padding: EdgeInsets.all(4),
              child: Icon(Icons.close, size: 14, color: ClawdTheme.clawLight),
            ),
          ),
        ],
      ),
    );
  }
}
