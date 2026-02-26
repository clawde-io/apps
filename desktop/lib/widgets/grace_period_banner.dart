import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:clawd_core/clawd_core.dart';

/// Wraps [child] and shows a persistent amber banner when the account is in
/// a dunning grace period (payment failed, subscription is still active but
/// expiring soon unless updated).
///
/// The banner:
///   - Shows "X days left to update payment" (or "Last day!")
///   - Has an "Update Payment" link that opens clawde.io/settings/billing
///   - Has a dismiss button (hides for the session only)
class GracePeriodBanner extends ConsumerStatefulWidget {
  const GracePeriodBanner({super.key, required this.child});

  final Widget child;

  @override
  ConsumerState<GracePeriodBanner> createState() => _GracePeriodBannerState();
}

class _GracePeriodBannerState extends ConsumerState<GracePeriodBanner> {
  bool _dismissed = false;

  Future<void> _openBilling() async {
    final uri = Uri.parse('https://clawde.io/settings/billing');
    if (await canLaunchUrl(uri)) {
      await launchUrl(uri, mode: LaunchMode.externalApplication);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_dismissed) {
      return Column(children: [Expanded(child: widget.child)]);
    }

    final licenseAsync = ref.watch(licenseProvider);

    return licenseAsync.when(
      loading: () => Column(children: [Expanded(child: widget.child)]),
      error: (_, __) => Column(children: [Expanded(child: widget.child)]),
      data: (license) {
        if (!license.inGracePeriod) {
          return Column(children: [Expanded(child: widget.child)]);
        }
        return Column(
          children: [
            _GracePeriodBannerStrip(
              daysRemaining: license.graceDaysRemaining!,
              onBillingTap: _openBilling,
              onDismiss: () => setState(() => _dismissed = true),
            ),
            Expanded(child: widget.child),
          ],
        );
      },
    );
  }
}

// ─── Banner strip ─────────────────────────────────────────────────────────────

class _GracePeriodBannerStrip extends StatelessWidget {
  const _GracePeriodBannerStrip({
    required this.daysRemaining,
    required this.onBillingTap,
    required this.onDismiss,
  });

  final int daysRemaining;
  final VoidCallback onBillingTap;
  final VoidCallback onDismiss;

  String get _message {
    if (daysRemaining <= 0) return 'Payment failed — access expires today.';
    if (daysRemaining == 1) return 'Last day! Payment failed — subscription expires tomorrow.';
    return 'Payment failed — $daysRemaining days left before access is downgraded.';
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      color: const Color(0xFF92400E).withValues(alpha: 0.25), // amber-800
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      child: Row(
        children: [
          const Icon(
            Icons.warning_amber_outlined,
            size: 14,
            color: Color(0xFFFBBF24), // amber-400
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              _message,
              style: const TextStyle(
                fontSize: 12,
                color: Color(0xFFFDE68A), // amber-200
                fontWeight: FontWeight.w500,
              ),
            ),
          ),
          TextButton(
            onPressed: onBillingTap,
            style: TextButton.styleFrom(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
              minimumSize: Size.zero,
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            ),
            child: const Text(
              'Update Payment',
              style: TextStyle(
                fontSize: 11,
                color: Color(0xFFFBBF24), // amber-400
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
              child: Icon(
                Icons.close,
                size: 14,
                color: Color(0xFFFBBF24), // amber-400
              ),
            ),
          ),
        ],
      ),
    );
  }
}
