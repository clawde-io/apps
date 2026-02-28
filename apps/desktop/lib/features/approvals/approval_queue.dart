import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Desktop approval queue — lists all pending agent approval requests
/// with full context. Each ApprovalCard shows Approve Once, Approve For
/// Task, Deny, and Clarify actions.
class ApprovalQueueScreen extends ConsumerWidget {
  const ApprovalQueueScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final approvalsAsync = ref.watch(approvalQueueProvider);
    final notifier = ref.read(approvalQueueProvider.notifier);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // ── Header ──────────────────────────────────────────────────────────
        Container(
          height: 56,
          padding: const EdgeInsets.symmetric(horizontal: 20),
          decoration: const BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            border: Border(bottom: BorderSide(color: ClawdTheme.surfaceBorder)),
          ),
          child: Row(
            children: [
              const Icon(Icons.approval_outlined, size: 16, color: Colors.amber),
              const SizedBox(width: 8),
              const Text(
                'Approval Queue',
                style: TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  color: Colors.white,
                ),
              ),
              const SizedBox(width: 8),
              approvalsAsync.when(
                data: (approvals) => approvals.isEmpty
                    ? const SizedBox.shrink()
                    : Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 8, vertical: 2),
                        decoration: BoxDecoration(
                          color: Colors.amber.withValues(alpha: 0.2),
                          borderRadius: BorderRadius.circular(10),
                        ),
                        child: Text(
                          '${approvals.length}',
                          style: const TextStyle(
                            fontSize: 11,
                            fontWeight: FontWeight.w600,
                            color: Colors.amber,
                          ),
                        ),
                      ),
                loading: () => const SizedBox.shrink(),
                error: (_, __) => const SizedBox.shrink(),
              ),
            ],
          ),
        ),

        // ── Approval list ───────────────────────────────────────────────────
        Expanded(
          child: approvalsAsync.when(
            loading: () => const Center(
              child: CircularProgressIndicator(color: ClawdTheme.claw),
            ),
            error: (e, _) => ErrorState(
              icon: Icons.error_outline,
              title: 'Failed to load approvals',
              description: e.toString(),
              onRetry: () => ref.refresh(approvalQueueProvider),
            ),
            data: (approvals) {
              if (approvals.isEmpty) {
                return const EmptyState(
                  icon: Icons.check_circle_outline,
                  title: 'No pending approvals',
                  subtitle: 'Agent actions that require your review will appear here.',
                );
              }

              return ListView.separated(
                padding: const EdgeInsets.all(16),
                itemCount: approvals.length,
                separatorBuilder: (_, __) => const SizedBox(height: 12),
                itemBuilder: (context, i) {
                  final request = approvals[i];
                  return ApprovalCard(
                    request: request,
                    onApprove: () => notifier.approve(request.approvalId),
                    onApproveForTask: () => notifier.approve(
                      request.approvalId,
                      forTask: true,
                    ),
                    onDeny: () => notifier.deny(request.approvalId),
                    onClarify: () => _showClarifyDialog(context, request.approvalId),
                  );
                },
              );
            },
          ),
        ),
      ],
    );
  }

  void _showClarifyDialog(BuildContext context, String approvalId) {
    showDialog<void>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: ClawdTheme.surfaceElevated,
        title: const Text(
          'Send Clarification',
          style: TextStyle(color: Colors.white, fontSize: 15),
        ),
        content: const Text(
          'Clarification messaging is not yet wired to the daemon. '
          'Use the chat window to send a message to the agent.',
          style: TextStyle(color: Colors.white70, fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('OK'),
          ),
        ],
      ),
    );
  }
}
