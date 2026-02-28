import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/features/chat/widgets/session_header.dart';
import 'package:clawde/features/chat/widgets/message_list.dart';
import 'package:clawde/features/chat/widgets/desktop_message_input.dart';
import 'package:clawde/features/chat/widgets/tool_call_panel.dart';

/// Claude Code context window budget: 200k tokens.
/// We estimate token usage client-side the same way the daemon does: chars/4 (min 1).
const _kMaxContextTokens = 200000;

int _estimateTokens(List<Message> messages) {
  if (messages.isEmpty) return 0;
  var chars = 0;
  for (final m in messages) {
    chars += m.content.length;
  }
  return (chars / 4).ceil().clamp(1, _kMaxContextTokens);
}

class ChatContent extends ConsumerStatefulWidget {
  const ChatContent({super.key});

  @override
  ConsumerState<ChatContent> createState() => _ChatContentState();
}

class _ChatContentState extends ConsumerState<ChatContent> {
  String _repoName(String path) {
    final parts = path.replaceAll(r'\', '/').split('/');
    return parts.where((p) => p.isNotEmpty).lastOrNull ?? path;
  }

  @override
  void initState() {
    super.initState();
    // Update window title when active session changes (DP-03). Registered
    // once in initState — never in build() — to avoid multiple subscriptions.
    ref.listenManual(activeSessionProvider, (_, next) {
      final title =
          next != null ? 'ClawDE — ${_repoName(next.repoPath)}' : 'ClawDE';
      windowManager.setTitle(title);
    });

    // V02.T12 — listen for session.contextOptimized push events and show toast.
    ref.listenManual(daemonPushEventsProvider, (_, next) {
      next.whenData((event) {
        final method = event['method'] as String?;
        if (method == 'session.contextOptimized') {
          _showContextToast();
        }
      });
    });
  }

  void _showContextToast() {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Row(
          children: [
            Icon(Icons.compress, size: 16, color: Colors.white70),
            SizedBox(width: 8),
            Text(
              'Context optimized — older messages summarized',
              style: TextStyle(fontSize: 12),
            ),
          ],
        ),
        backgroundColor: Color(0xFF1E2030),
        behavior: SnackBarBehavior.floating,
        margin: EdgeInsets.only(bottom: 48, left: 16, right: 16),
        duration: Duration(seconds: 4),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final session = ref.watch(activeSessionProvider);

    if (session == null) {
      return const EmptyState(
        icon: Icons.chat_bubble_outline,
        title: 'Select a session',
        subtitle: 'Choose from the sidebar or create a new one',
      );
    }

    // V02.T12 — estimate context usage from loaded messages.
    final messages =
        ref.watch(messageListProvider(session.id)).valueOrNull ?? [];
    final estimatedTokens = _estimateTokens(messages);
    final contextPct = (estimatedTokens / _kMaxContextTokens * 100).round();

    // MI.T14 — token usage panel data
    final tokenUsage =
        ref.watch(tokenSessionUsageProvider(session.id)).valueOrNull;
    final budgetStatus = ref.watch(tokenBudgetStatusProvider).valueOrNull;

    return Stack(
      children: [
        Column(
          children: [
            SessionHeader(session: session),
            // V02.T12 — context usage bar (only shown when >40% full)
            if (contextPct > 40)
              Padding(
                padding: const EdgeInsets.symmetric(
                    horizontal: 12, vertical: 4),
                child: ContextBudgetBar(
                  currentTokens: estimatedTokens,
                  maxTokens: _kMaxContextTokens,
                  height: 4,
                  showLabel: contextPct > 70,
                ),
              ),
            Expanded(child: MessageList(sessionId: session.id)),
            ToolCallPanel(sessionId: session.id),
            // MI.T14 — token usage panel (always visible, expands on warning)
            TokenUsagePanel(
              inputTokens:
                  (tokenUsage?['inputTokens'] as num?)?.toInt() ?? 0,
              outputTokens:
                  (tokenUsage?['outputTokens'] as num?)?.toInt() ?? 0,
              estimatedCostUsd:
                  (tokenUsage?['estimatedCostUsd'] as num?)?.toDouble() ??
                      0.0,
              contextPercent: contextPct.toDouble(),
              monthlySpendUsd:
                  (budgetStatus?['monthlySpendUsd'] as num?)?.toDouble() ??
                      0.0,
              monthlyCap:
                  (budgetStatus?['cap'] as num?)?.toDouble(),
              budgetWarning:
                  budgetStatus?['warning'] as bool? ?? false,
              budgetExceeded:
                  budgetStatus?['exceeded'] as bool? ?? false,
            ),
            DesktopMessageInput(sessionId: session.id, session: session),
          ],
        ),
        // V02.T10 — cold session resume overlay
        if (session.tier == SessionTier.cold &&
            session.status == SessionStatus.running)
          const _ResumeOverlay(),
      ],
    );
  }
}

/// V02.T10 — "Resuming..." overlay shown when a cold session begins running.
class _ResumeOverlay extends StatelessWidget {
  const _ResumeOverlay();

  @override
  Widget build(BuildContext context) {
    return Positioned.fill(
      child: Container(
        color: Colors.black.withValues(alpha: 0.55),
        child: const Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              SizedBox(
                width: 32,
                height: 32,
                child: CircularProgressIndicator(
                  strokeWidth: 2.5,
                  color: Colors.white70,
                ),
              ),
              SizedBox(height: 16),
              Text(
                'Resuming session...',
                style: TextStyle(
                  fontSize: 15,
                  fontWeight: FontWeight.w600,
                  color: Colors.white,
                ),
              ),
              SizedBox(height: 6),
              Text(
                'Loading context and warming up the runner',
                style: TextStyle(
                  fontSize: 12,
                  color: Colors.white54,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
