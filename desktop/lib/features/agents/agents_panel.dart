import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Desktop sidebar panel listing all active agents with role, status,
/// provider, token usage, cost estimate, and heartbeat age.
class AgentsPanel extends ConsumerWidget {
  const AgentsPanel({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final agentsAsync = ref.watch(agentsProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // ── Header ──────────────────────────────────────────────────────────
        Container(
          height: 48,
          padding: const EdgeInsets.symmetric(horizontal: 16),
          decoration: const BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            border: Border(bottom: BorderSide(color: ClawdTheme.surfaceBorder)),
          ),
          child: Row(
            children: [
              const Icon(Icons.smart_toy_outlined, size: 14, color: ClawdTheme.clawLight),
              const SizedBox(width: 8),
              const Text(
                'Agents',
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w700,
                  color: Colors.white,
                ),
              ),
              const SizedBox(width: 6),
              agentsAsync.when(
                data: (agents) => _CountBadge(count: agents.length),
                loading: () => const SizedBox.shrink(),
                error: (_, __) => const SizedBox.shrink(),
              ),
              const Spacer(),
              IconButton(
                icon: const Icon(Icons.refresh, size: 14, color: Colors.white38),
                tooltip: 'Refresh',
                onPressed: () => ref.refresh(agentsProvider),
                padding: const EdgeInsets.all(4),
                constraints: const BoxConstraints(),
              ),
            ],
          ),
        ),

        // ── Agent list ──────────────────────────────────────────────────────
        Expanded(
          child: agentsAsync.when(
            loading: () => const Center(
              child: CircularProgressIndicator(
                color: ClawdTheme.claw,
                strokeWidth: 2,
              ),
            ),
            error: (e, _) => ErrorState(
              icon: Icons.error_outline,
              title: 'Failed to load agents',
              description: e.toString(),
              onRetry: () => ref.refresh(agentsProvider),
            ),
            data: (agents) {
              if (agents.isEmpty) {
                return const EmptyState(
                  icon: Icons.smart_toy_outlined,
                  title: 'No active agents',
                  subtitle: 'Agents will appear here once a multi-agent task starts.',
                );
              }
              return ListView.builder(
                padding: const EdgeInsets.symmetric(vertical: 6),
                itemCount: agents.length,
                itemBuilder: (context, i) => _AgentRow(agent: agents[i]),
              );
            },
          ),
        ),
      ],
    );
  }
}

// ── Agent row ──────────────────────────────────────────────────────────────────

class _AgentRow extends StatelessWidget {
  const _AgentRow({required this.agent});
  final AgentView agent;

  Color _statusColor(AgentViewStatus s) => switch (s) {
        AgentViewStatus.active => Colors.green,
        AgentViewStatus.idle => Colors.amber,
        AgentViewStatus.offline => Colors.white38,
      };

  String _timeSince(int? unixSec) {
    if (unixSec == null) return '—';
    final dt = DateTime.fromMillisecondsSinceEpoch(unixSec * 1000);
    final diff = DateTime.now().difference(dt);
    if (diff.inSeconds < 60) return '${diff.inSeconds}s';
    if (diff.inMinutes < 60) return '${diff.inMinutes}m';
    return '${diff.inHours}h';
  }

  @override
  Widget build(BuildContext context) {
    final statusColor = _statusColor(agent.status);

    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 10, vertical: 3),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(7),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Type + status
          Row(
            children: [
              _TypeBadge(agentType: agent.agentType),
              const Spacer(),
              Container(
                width: 7,
                height: 7,
                decoration: BoxDecoration(
                  color: statusColor,
                  shape: BoxShape.circle,
                ),
              ),
              const SizedBox(width: 5),
              Text(
                agent.status.name,
                style: TextStyle(
                  fontSize: 10,
                  fontWeight: FontWeight.w600,
                  color: statusColor,
                ),
              ),
            ],
          ),
          const SizedBox(height: 5),

          // Task ID + provider
          Row(
            children: [
              Text(
                'Task ${agent.currentTaskId ?? '—'}',
                style: const TextStyle(fontSize: 10, color: Colors.white54),
              ),
              const SizedBox(width: 8),
              ProviderBadge(
                provider: agent.agentType == 'claude'
                    ? ProviderType.claude
                    : ProviderType.codex,
              ),
            ],
          ),
          const SizedBox(height: 4),

          // Last seen
          Row(
            children: [
              const Icon(Icons.favorite_border, size: 10, color: Colors.white38),
              const SizedBox(width: 3),
              Text(
                _timeSince(agent.lastSeen),
                style: const TextStyle(fontSize: 10, color: Colors.white38),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

// ── Sub-widgets ────────────────────────────────────────────────────────────────

class _TypeBadge extends StatelessWidget {
  const _TypeBadge({required this.agentType});
  final String agentType;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: ClawdTheme.claw.withValues(alpha: 0.2),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        agentType,
        style: const TextStyle(
          fontSize: 10,
          fontWeight: FontWeight.w700,
          color: ClawdTheme.clawLight,
        ),
      ),
    );
  }
}

class _CountBadge extends StatelessWidget {
  const _CountBadge({required this.count});
  final int count;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
      decoration: BoxDecoration(
        color: ClawdTheme.claw.withValues(alpha: 0.2),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Text(
        '$count',
        style: const TextStyle(
          fontSize: 10,
          fontWeight: FontWeight.w600,
          color: ClawdTheme.clawLight,
        ),
      ),
    );
  }
}
