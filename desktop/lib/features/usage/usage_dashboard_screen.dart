import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Standalone usage dashboard (MI.T16).
///
/// Shows:
/// - Monthly total spend + budget remaining
/// - Per-model breakdown table
/// - Per-session cost breakdown (from loaded sessions + token.sessionUsage)
class UsageDashboardScreen extends ConsumerWidget {
  const UsageDashboardScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final totalUsageAsync = ref.watch(tokenTotalUsageProvider);
    final budgetAsync = ref.watch(tokenBudgetStatusProvider);
    final sessionsAsync = ref.watch(sessionListProvider);

    return Scaffold(
      backgroundColor: ClawdTheme.surface,
      appBar: AppBar(
        backgroundColor: ClawdTheme.surfaceElevated,
        title: const Row(
          children: [
            Icon(Icons.receipt_long, size: 18, color: Colors.amber),
            SizedBox(width: 10),
            Text(
              'Usage Dashboard',
              style: TextStyle(fontSize: 15, fontWeight: FontWeight.w700),
            ),
          ],
        ),
        elevation: 0,
        bottom: PreferredSize(
          preferredSize: const Size.fromHeight(1),
          child: Container(height: 1, color: ClawdTheme.surfaceBorder),
        ),
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // ── Budget summary card ─────────────────────────────────────────
            budgetAsync.when(
              loading: () => const LinearProgressIndicator(),
              error: (_, __) => const _UnavailableBanner(
                  'Budget data unavailable — daemon may be disconnected'),
              data: (budget) => _BudgetCard(budget: budget),
            ),
            const SizedBox(height: 24),

            // ── Per-model breakdown ─────────────────────────────────────────
            const _SectionHeader('Breakdown by model (current month)'),
            const SizedBox(height: 12),
            totalUsageAsync.when(
              loading: () => const LinearProgressIndicator(),
              error: (_, __) => const _UnavailableBanner(
                  'Usage data unavailable — daemon may be disconnected'),
              data: (rows) => rows.isEmpty
                  ? const _EmptyRow('No usage recorded this month')
                  : _ModelBreakdownTable(rows: rows),
            ),
            const SizedBox(height: 24),

            // ── Per-session table ───────────────────────────────────────────
            const _SectionHeader('Per-session cost (current sessions)'),
            const SizedBox(height: 12),
            sessionsAsync.when(
              loading: () => const LinearProgressIndicator(),
              error: (e, _) =>
                  _UnavailableBanner('Could not load sessions: $e'),
              data: (sessions) => sessions.isEmpty
                  ? const _EmptyRow('No sessions found')
                  : _SessionCostTable(sessions: sessions, ref: ref),
            ),
          ],
        ),
      ),
    );
  }
}

// ── Budget summary card ────────────────────────────────────────────────────────

class _BudgetCard extends StatelessWidget {
  const _BudgetCard({required this.budget});
  final Map<String, dynamic>? budget;

  @override
  Widget build(BuildContext context) {
    final spend =
        (budget?['monthlySpendUsd'] as num?)?.toDouble() ?? 0.0;
    final cap = (budget?['cap'] as num?)?.toDouble();
    final pct = (budget?['pct'] as num?)?.toDouble() ?? 0.0;
    final warning = budget?['warning'] as bool? ?? false;
    final exceeded = budget?['exceeded'] as bool? ?? false;

    final statusColor = exceeded
        ? ClawdTheme.error
        : warning
            ? ClawdTheme.warning
            : ClawdTheme.success;

    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Text(
                'Monthly spend',
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: Colors.white70,
                ),
              ),
              const Spacer(),
              if (exceeded)
                const _StatusBadge('Budget exceeded', ClawdTheme.error)
              else if (warning)
                const _StatusBadge('Budget warning', ClawdTheme.warning)
              else if (cap != null)
                const _StatusBadge('Within budget', ClawdTheme.success),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text(
                '\$${spend.toStringAsFixed(4)}',
                style: const TextStyle(
                  fontSize: 28,
                  fontWeight: FontWeight.w700,
                  color: Colors.white,
                ),
              ),
              if (cap != null) ...[
                const SizedBox(width: 8),
                Padding(
                  padding: const EdgeInsets.only(bottom: 4),
                  child: Text(
                    'of \$${cap.toStringAsFixed(2)} cap',
                    style: const TextStyle(
                      fontSize: 13,
                      color: Colors.white38,
                    ),
                  ),
                ),
              ],
            ],
          ),
          if (cap != null) ...[
            const SizedBox(height: 12),
            ClipRRect(
              borderRadius: BorderRadius.circular(3),
              child: SizedBox(
                height: 6,
                child: Stack(
                  children: [
                    Container(color: Colors.white12),
                    FractionallySizedBox(
                      widthFactor: (pct / 100).clamp(0.0, 1.0),
                      child: Container(color: statusColor),
                    ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 6),
            Text(
              '${pct.toStringAsFixed(1)}% of budget used',
              style:
                  TextStyle(fontSize: 11, color: statusColor),
            ),
          ],
        ],
      ),
    );
  }
}

// ── Model breakdown table ──────────────────────────────────────────────────────

class _ModelBreakdownTable extends StatelessWidget {
  const _ModelBreakdownTable({required this.rows});
  final List<Map<String, dynamic>> rows;

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Column(
        children: [
          // Header row
          const Padding(
            padding: EdgeInsets.symmetric(horizontal: 16, vertical: 10),
            child: Row(
              children: [
                Expanded(
                    flex: 3,
                    child: Text('Model',
                        style: TextStyle(
                            fontSize: 11,
                            fontWeight: FontWeight.w600,
                            color: Colors.white38))),
                Expanded(
                    child: Text('Input',
                        style: TextStyle(
                            fontSize: 11,
                            fontWeight: FontWeight.w600,
                            color: Colors.white38))),
                Expanded(
                    child: Text('Output',
                        style: TextStyle(
                            fontSize: 11,
                            fontWeight: FontWeight.w600,
                            color: Colors.white38))),
                Expanded(
                    child: Text('Cost',
                        textAlign: TextAlign.right,
                        style: TextStyle(
                            fontSize: 11,
                            fontWeight: FontWeight.w600,
                            color: Colors.white38))),
              ],
            ),
          ),
          const Divider(height: 1),
          ...rows.asMap().entries.map((e) {
            final row = e.value;
            final isLast = e.key == rows.length - 1;
            final modelId = row['modelId'] as String? ?? '—';
            final input = (row['inputTokens'] as num?)?.toInt() ?? 0;
            final output = (row['outputTokens'] as num?)?.toInt() ?? 0;
            final cost =
                (row['estimatedCostUsd'] as num?)?.toDouble() ?? 0.0;
            return Column(
              children: [
                Padding(
                  padding: const EdgeInsets.symmetric(
                      horizontal: 16, vertical: 10),
                  child: Row(
                    children: [
                      Expanded(
                        flex: 3,
                        child: Text(
                          _shortModelId(modelId),
                          style: const TextStyle(
                            fontSize: 12,
                            color: Colors.white70,
                          ),
                        ),
                      ),
                      Expanded(
                          child: Text(_fmtTokens(input),
                              style: const TextStyle(
                                  fontSize: 12, color: Colors.white54))),
                      Expanded(
                          child: Text(_fmtTokens(output),
                              style: const TextStyle(
                                  fontSize: 12, color: Colors.white54))),
                      Expanded(
                          child: Text(
                        '\$${cost.toStringAsFixed(4)}',
                        textAlign: TextAlign.right,
                        style: const TextStyle(
                          fontSize: 12,
                          color: Colors.white70,
                          fontWeight: FontWeight.w500,
                        ),
                      )),
                    ],
                  ),
                ),
                if (!isLast) const Divider(height: 1, indent: 16),
              ],
            );
          }),
        ],
      ),
    );
  }

  static String _fmtTokens(int n) {
    if (n >= 1000) {
      final k = n / 1000;
      return k < 100 ? '${k.toStringAsFixed(1)}k' : '${k.round()}k';
    }
    return n.toString();
  }

  static String _shortModelId(String id) {
    for (final name in ['opus', 'sonnet', 'haiku']) {
      if (id.toLowerCase().contains(name)) {
        return 'Claude ${name[0].toUpperCase()}${name.substring(1)}';
      }
    }
    return id.length > 30 ? '${id.substring(0, 30)}…' : id;
  }
}

// ── Per-session cost table ─────────────────────────────────────────────────────

class _SessionCostTable extends ConsumerWidget {
  const _SessionCostTable({
    required this.sessions,
    required this.ref,
  });
  final List sessions;
  final WidgetRef ref;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Column(
        children: [
          const Padding(
            padding: EdgeInsets.symmetric(horizontal: 16, vertical: 10),
            child: Row(
              children: [
                Expanded(
                    flex: 3,
                    child: Text('Session',
                        style: TextStyle(
                            fontSize: 11,
                            fontWeight: FontWeight.w600,
                            color: Colors.white38))),
                Expanded(
                    child: Text('Messages',
                        style: TextStyle(
                            fontSize: 11,
                            fontWeight: FontWeight.w600,
                            color: Colors.white38))),
                Expanded(
                    child: Text('Cost',
                        textAlign: TextAlign.right,
                        style: TextStyle(
                            fontSize: 11,
                            fontWeight: FontWeight.w600,
                            color: Colors.white38))),
              ],
            ),
          ),
          const Divider(height: 1),
          ...sessions.asMap().entries.map((e) {
            final session = e.value;
            final isLast = e.key == sessions.length - 1;
            return _SessionRow(
              session: session,
              isLast: isLast,
            );
          }),
        ],
      ),
    );
  }
}

class _SessionRow extends ConsumerWidget {
  const _SessionRow({required this.session, required this.isLast});
  final dynamic session;
  final bool isLast;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final usage =
        ref.watch(tokenSessionUsageProvider(session.id)).valueOrNull;
    final cost =
        (usage?['estimatedCostUsd'] as num?)?.toDouble() ?? 0.0;
    final msgCount = (usage?['messageCount'] as num?)?.toInt() ??
        (session.messageCount as int? ?? 0);
    final repoName =
        (session.repoPath as String).split('/').last;

    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(
              horizontal: 16, vertical: 10),
          child: Row(
            children: [
              Expanded(
                flex: 3,
                child: Text(
                  repoName,
                  style: const TextStyle(
                    fontSize: 12,
                    color: Colors.white70,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              Expanded(
                child: Text(
                  '$msgCount',
                  style: const TextStyle(
                      fontSize: 12, color: Colors.white54),
                ),
              ),
              Expanded(
                child: Text(
                  cost > 0 ? '\$${cost.toStringAsFixed(4)}' : '—',
                  textAlign: TextAlign.right,
                  style: const TextStyle(
                    fontSize: 12,
                    color: Colors.white70,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
            ],
          ),
        ),
        if (!isLast) const Divider(height: 1, indent: 16),
      ],
    );
  }
}

// ── Shared helpers ─────────────────────────────────────────────────────────────

class _SectionHeader extends StatelessWidget {
  const _SectionHeader(this.title);
  final String title;

  @override
  Widget build(BuildContext context) {
    return Text(
      title,
      style: const TextStyle(
        fontSize: 13,
        fontWeight: FontWeight.w700,
        color: Colors.white70,
      ),
    );
  }
}

class _StatusBadge extends StatelessWidget {
  const _StatusBadge(this.label, this.color);
  final String label;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontSize: 10,
          color: color,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}

class _EmptyRow extends StatelessWidget {
  const _EmptyRow(this.message);
  final String message;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(24),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Center(
        child: Text(
          message,
          style: const TextStyle(fontSize: 12, color: Colors.white38),
        ),
      ),
    );
  }
}

class _UnavailableBanner extends StatelessWidget {
  const _UnavailableBanner(this.message);
  final String message;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: ClawdTheme.error.withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.error.withValues(alpha: 0.3)),
      ),
      child: Row(
        children: [
          const Icon(Icons.warning_amber_rounded,
              size: 14, color: ClawdTheme.error),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              message,
              style: const TextStyle(
                  fontSize: 12, color: ClawdTheme.error),
            ),
          ),
        ],
      ),
    );
  }
}
