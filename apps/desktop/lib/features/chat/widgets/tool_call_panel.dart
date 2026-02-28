import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

class ToolCallPanel extends ConsumerStatefulWidget {
  const ToolCallPanel({super.key, required this.sessionId});

  final String sessionId;

  @override
  ConsumerState<ToolCallPanel> createState() => _ToolCallPanelState();
}

class _ToolCallPanelState extends ConsumerState<ToolCallPanel> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final toolCallsAsync = ref.watch(toolCallProvider(widget.sessionId));
    final pendingCalls = toolCallsAsync.valueOrNull ?? [];
    final count = pendingCalls.length;

    if (count == 0) return const SizedBox.shrink();

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        // Collapsible amber header
        InkWell(
          onTap: () => setState(() => _expanded = !_expanded),
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
            decoration: const BoxDecoration(
              color: ClawdTheme.surfaceElevated,
              border: Border(
                top: BorderSide(color: ClawdTheme.surfaceBorder),
                bottom: BorderSide(color: ClawdTheme.surfaceBorder),
              ),
            ),
            child: Row(
              children: [
                const Icon(Icons.handyman, size: 16, color: ClawdTheme.warning),
                const SizedBox(width: 8),
                Text(
                  '$count tool call${count == 1 ? '' : 's'} awaiting approval',
                  style: const TextStyle(
                    fontSize: 13,
                    color: ClawdTheme.warning,
                    fontWeight: FontWeight.w500,
                  ),
                ),
                const Spacer(),
                Icon(
                  _expanded ? Icons.expand_less : Icons.expand_more,
                  size: 18,
                  color: ClawdTheme.warning,
                ),
              ],
            ),
          ),
        ),
        // Expandable tool call list
        if (_expanded)
          ConstrainedBox(
            constraints: const BoxConstraints(maxHeight: 300),
            child: ListView.builder(
              shrinkWrap: true,
              itemCount: pendingCalls.length,
              itemBuilder: (context, i) {
                final tc = pendingCalls[i];
                final notifier =
                    ref.read(toolCallProvider(widget.sessionId).notifier);
                return ToolCallCard(
                  toolCall: tc,
                  onApprove: () => notifier.approve(tc.id),
                  onReject: () => notifier.reject(tc.id),
                );
              },
            ),
          ),
      ],
    );
  }
}
