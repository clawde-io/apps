import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Mobile approval queue — touch-optimised with swipe-to-approve/deny.
/// Swipe right = Approve, swipe left = Deny.
/// Tap to see full approval detail in a bottom sheet.
class MobileApprovalQueueScreen extends ConsumerWidget {
  const MobileApprovalQueueScreen({super.key});

  Future<void> _refresh(WidgetRef ref) async {
    await ref.read(approvalQueueProvider.notifier).refresh();
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final approvalsAsync = ref.watch(approvalQueueProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Approvals'),
        actions: [
          approvalsAsync.when(
            data: (approvals) => approvals.isEmpty
                ? const SizedBox.shrink()
                : Container(
                    margin: const EdgeInsets.only(right: 12),
                    padding: const EdgeInsets.symmetric(
                        horizontal: 8, vertical: 3),
                    decoration: BoxDecoration(
                      color: Colors.amber.withValues(alpha: 0.2),
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Text(
                      '${approvals.length}',
                      style: const TextStyle(
                        fontSize: 12,
                        fontWeight: FontWeight.w700,
                        color: Colors.amber,
                      ),
                    ),
                  ),
            loading: () => const SizedBox.shrink(),
            error: (_, __) => const SizedBox.shrink(),
          ),
        ],
      ),
      body: approvalsAsync.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => ErrorState(
          icon: Icons.error_outline,
          title: 'Failed to load approvals',
          description: e.toString(),
          onRetry: () => _refresh(ref),
        ),
        data: (approvals) {
          if (approvals.isEmpty) {
            return const EmptyState(
              icon: Icons.check_circle_outline,
              title: 'No pending approvals',
              subtitle: 'Agent actions that require your review will appear here.',
            );
          }

          return RefreshIndicator(
            onRefresh: () => _refresh(ref),
            child: ListView.separated(
              physics: const AlwaysScrollableScrollPhysics(),
              padding: const EdgeInsets.symmetric(vertical: 8),
              itemCount: approvals.length,
              separatorBuilder: (_, __) => const SizedBox(height: 2),
              itemBuilder: (context, i) {
                final request = approvals[i];
                return _SwipeableApprovalItem(request: request);
              },
            ),
          );
        },
      ),
    );
  }
}

// ── Swipeable item ─────────────────────────────────────────────────────────────

class _SwipeableApprovalItem extends ConsumerWidget {
  const _SwipeableApprovalItem({required this.request});
  final ApprovalRequest request;

  void _showDetail(BuildContext context, WidgetRef ref) {
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: ClawdTheme.surfaceElevated,
      isScrollControlled: true,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (ctx) => _ApprovalDetailSheet(
        request: request,
        onApprove: () {
          ref.read(approvalQueueProvider.notifier).approve(request.approvalId);
          Navigator.of(ctx).pop();
        },
        onApproveForTask: () {
          ref.read(approvalQueueProvider.notifier).approve(
            request.approvalId,
            forTask: true,
          );
          Navigator.of(ctx).pop();
        },
        onDeny: () {
          ref.read(approvalQueueProvider.notifier).deny(request.approvalId);
          Navigator.of(ctx).pop();
        },
      ),
    );
  }

  Color get _riskColor => switch (request.risk) {
        'low' => Colors.green,
        'medium' => Colors.amber,
        'high' => Colors.orange,
        'critical' => Colors.red,
        _ => Colors.amber,
      };

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final notifier = ref.read(approvalQueueProvider.notifier);

    return Dismissible(
      key: ValueKey(request.approvalId),
      // Swipe right → Approve
      background: Container(
        alignment: Alignment.centerLeft,
        padding: const EdgeInsets.only(left: 24),
        color: Colors.green.withValues(alpha: 0.15),
        child: const Row(
          children: [
            Icon(Icons.check_circle, color: Colors.green, size: 24),
            SizedBox(width: 8),
            Text(
              'Approve',
              style: TextStyle(
                color: Colors.green,
                fontWeight: FontWeight.w700,
                fontSize: 14,
              ),
            ),
          ],
        ),
      ),
      // Swipe left → Deny
      secondaryBackground: Container(
        alignment: Alignment.centerRight,
        padding: const EdgeInsets.only(right: 24),
        color: Colors.red.withValues(alpha: 0.15),
        child: const Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            Text(
              'Deny',
              style: TextStyle(
                color: Colors.red,
                fontWeight: FontWeight.w700,
                fontSize: 14,
              ),
            ),
            SizedBox(width: 8),
            Icon(Icons.cancel, color: Colors.red, size: 24),
          ],
        ),
      ),
      confirmDismiss: (direction) async {
        if (direction == DismissDirection.startToEnd) {
          await notifier.approve(request.approvalId);
          return true;
        } else {
          await notifier.deny(request.approvalId);
          return true;
        }
      },
      child: InkWell(
        onTap: () => _showDetail(context, ref),
        child: Container(
          margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
          padding: const EdgeInsets.all(14),
          decoration: BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            borderRadius: BorderRadius.circular(10),
            border: Border.all(
              color: _riskColor.withValues(alpha: 0.4),
            ),
          ),
          child: Row(
            children: [
              // Risk indicator bar
              Container(
                width: 4,
                height: 44,
                decoration: BoxDecoration(
                  color: _riskColor,
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Text(
                          request.tool,
                          style: const TextStyle(
                            fontSize: 13,
                            fontWeight: FontWeight.w700,
                            color: Colors.white,
                          ),
                        ),
                        const Spacer(),
                        _RiskChip(risk: request.risk, color: _riskColor),
                      ],
                    ),
                    const SizedBox(height: 3),
                    Text(
                      request.argsSummary,
                      style: const TextStyle(
                          fontSize: 11, color: Colors.white54),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 3),
                    Text(
                      'Task ${request.taskId}',
                      style: const TextStyle(
                          fontSize: 10, color: Colors.white38),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 8),
              const Icon(Icons.chevron_right, color: Colors.white38, size: 18),
            ],
          ),
        ),
      ),
    );
  }
}

// ── Risk chip ──────────────────────────────────────────────────────────────────

class _RiskChip extends StatelessWidget {
  const _RiskChip({required this.risk, required this.color});
  final String risk;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        risk.toUpperCase(),
        style: TextStyle(
          fontSize: 9,
          fontWeight: FontWeight.w700,
          color: color,
          letterSpacing: 0.5,
        ),
      ),
    );
  }
}

// ── Bottom sheet detail ────────────────────────────────────────────────────────

class _ApprovalDetailSheet extends StatelessWidget {
  const _ApprovalDetailSheet({
    required this.request,
    required this.onApprove,
    required this.onApproveForTask,
    required this.onDeny,
  });
  final ApprovalRequest request;
  final VoidCallback onApprove;
  final VoidCallback onApproveForTask;
  final VoidCallback onDeny;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: EdgeInsets.only(
        bottom: MediaQuery.of(context).viewInsets.bottom,
      ),
      child: SingleChildScrollView(
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              // Handle
              Center(
                child: Container(
                  width: 36,
                  height: 4,
                  decoration: BoxDecoration(
                    color: Colors.white24,
                    borderRadius: BorderRadius.circular(2),
                  ),
                ),
              ),
              const SizedBox(height: 16),

              // Full ApprovalCard
              ApprovalCard(
                request: request,
                onApprove: onApprove,
                onApproveForTask: onApproveForTask,
                onDeny: onDeny,
              ),
              const SizedBox(height: 8),
            ],
          ),
        ),
      ),
    );
  }
}
