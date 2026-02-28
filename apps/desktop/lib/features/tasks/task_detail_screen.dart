// SPDX-License-Identifier: MIT
/// Sprint ZZ EP.T05 — Task detail screen with evidence tab.
///
/// Shows full task metadata (Overview tab) and the evidence pack
/// collected during task execution (Evidence tab).
library;

import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

// ─── Task Detail Screen ───────────────────────────────────────────────────────

/// Full-page task detail view with Overview + Evidence tabs.
///
/// Pass [taskId] as route argument: `context.push('/tasks/${task.id}')`.
class TaskDetailScreen extends ConsumerWidget {
  const TaskDetailScreen({super.key, required this.taskId});

  final String taskId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tasksAsync =
        ref.watch(taskListProvider(const TaskFilter()));

    final task = tasksAsync.valueOrNull
        ?.where((t) => t.id == taskId)
        .firstOrNull;

    return DefaultTabController(
      length: 2,
      child: Column(
        children: [
          _Header(task: task, taskId: taskId),
          const TabBar(
            tabs: [
              Tab(text: 'Overview'),
              Tab(text: 'Evidence'),
            ],
            labelColor: ClawdTheme.claw,
            unselectedLabelColor: Colors.white38,
            indicatorColor: ClawdTheme.claw,
          ),
          Expanded(
            child: TabBarView(
              children: [
                _OverviewTab(task: task),
                _EvidenceTab(taskId: taskId),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

// ─── Header ───────────────────────────────────────────────────────────────────

class _Header extends StatelessWidget {
  const _Header({required this.task, required this.taskId});

  final AgentTask? task;
  final String taskId;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
      decoration: const BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        border: Border(bottom: BorderSide(color: ClawdTheme.surfaceBorder)),
      ),
      child: Row(
        children: [
          IconButton(
            icon: const Icon(Icons.arrow_back_ios_new, size: 16),
            color: Colors.white54,
            onPressed: () => Navigator.of(context).pop(),
            tooltip: 'Back',
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              task?.title ?? taskId,
              style: const TextStyle(
                fontSize: 15,
                fontWeight: FontWeight.w600,
                color: Colors.white,
              ),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          if (task != null) TaskStatusBadge(status: task!.status),
        ],
      ),
    );
  }
}

// ─── Overview tab ─────────────────────────────────────────────────────────────

class _OverviewTab extends StatelessWidget {
  const _OverviewTab({required this.task});

  final AgentTask? task;

  @override
  Widget build(BuildContext context) {
    if (task == null) {
      return const Center(
        child: Text('Task not found',
            style: TextStyle(fontSize: 13, color: Colors.white38)),
      );
    }
    final t = task!;

    return ListView(
      padding: const EdgeInsets.all(20),
      children: [
        if (t.description != null && t.description!.isNotEmpty) ...[
          const _SectionLabel('Description'),
          const SizedBox(height: 6),
          Text(t.description!,
              style: const TextStyle(fontSize: 13, color: Colors.white70)),
          const SizedBox(height: 16),
        ],
        if (t.phase != null) ...[
          const _SectionLabel('Phase'),
          const SizedBox(height: 4),
          _InfoChip(t.phase!),
          const SizedBox(height: 16),
        ],
        if (t.claimedBy != null) ...[
          const _SectionLabel('Agent'),
          const SizedBox(height: 4),
          _InfoChip(t.claimedBy!),
          const SizedBox(height: 16),
        ],
        if (t.files.isNotEmpty) ...[
          const _SectionLabel('Files'),
          const SizedBox(height: 6),
          ...t.files.map((f) => Padding(
                padding: const EdgeInsets.only(bottom: 4),
                child: Row(
                  children: [
                    const Icon(Icons.insert_drive_file_outlined,
                        size: 12, color: Colors.white38),
                    const SizedBox(width: 6),
                    Expanded(
                      child: Text(f,
                          style: const TextStyle(
                              fontSize: 11, color: Colors.white54),
                          overflow: TextOverflow.ellipsis),
                    ),
                  ],
                ),
              )),
          const SizedBox(height: 16),
        ],
        if (t.notes != null) ...[
          const _SectionLabel('Notes'),
          const SizedBox(height: 6),
          Text(t.notes!,
              style: const TextStyle(fontSize: 12, color: Colors.white54)),
          const SizedBox(height: 16),
        ],
        if (t.blockReason != null) ...[
          const _SectionLabel('Blocked reason'),
          const SizedBox(height: 6),
          Container(
            padding:
                const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            decoration: BoxDecoration(
              color: ClawdTheme.error.withValues(alpha: 0.08),
              borderRadius: BorderRadius.circular(6),
              border:
                  Border.all(color: ClawdTheme.error.withValues(alpha: 0.3)),
            ),
            child: Text(t.blockReason!,
                style: const TextStyle(
                    fontSize: 12, color: ClawdTheme.error)),
          ),
        ],
        // Timeline
        const SizedBox(height: 16),
        const _SectionLabel('Timeline'),
        const SizedBox(height: 8),
        _TimelineRow(
            label: 'Created', ts: t.createdAt),
        if (t.claimedAt != null)
          _TimelineRow(label: 'Claimed', ts: t.claimedAt),
        if (t.startedAt != null)
          _TimelineRow(label: 'Started', ts: t.startedAt),
        if (t.completedAt != null)
          _TimelineRow(label: 'Completed', ts: t.completedAt),
      ],
    );
  }
}

// ─── Evidence tab ─────────────────────────────────────────────────────────────

class _EvidenceTab extends ConsumerStatefulWidget {
  const _EvidenceTab({required this.taskId});

  final String taskId;

  @override
  ConsumerState<_EvidenceTab> createState() => _EvidenceTabState();
}

class _EvidenceTabState extends ConsumerState<_EvidenceTab> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref
          .read(evidencePackProvider(widget.taskId).notifier)
          .load();
    });
  }

  @override
  Widget build(BuildContext context) {
    final async = ref.watch(evidencePackProvider(widget.taskId));

    return async.when(
      loading: () => const Center(
        child: CircularProgressIndicator(color: ClawdTheme.claw),
      ),
      error: (e, _) => Center(
        child: ErrorState(
          icon: Icons.error_outline,
          title: 'Evidence unavailable',
          description: e.toString(),
          onRetry: () =>
              ref.read(evidencePackProvider(widget.taskId).notifier).load(),
        ),
      ),
      data: (pack) {
        if (pack == null) {
          return const Center(
            child: EmptyState(
              icon: Icons.assignment_outlined,
              title: 'No evidence pack',
              subtitle:
                  'Evidence is collected when a task completes with daemon tracking enabled.',
            ),
          );
        }
        return ListView(
          padding: const EdgeInsets.all(20),
          children: [
            _EvidenceMetaCard(pack: pack),
            const SizedBox(height: 12),
            _DiffStatsCard(stats: pack.diffStats),
            const SizedBox(height: 12),
            _TestResultsCard(results: pack.testResults),
            const SizedBox(height: 12),
            _ToolTraceCard(trace: pack.toolTrace),
            if (pack.reviewerVerdict != null) ...[
              const SizedBox(height: 12),
              _ReviewerVerdictCard(verdict: pack.reviewerVerdict!),
            ],
          ],
        );
      },
    );
  }
}

// ─── Evidence meta ────────────────────────────────────────────────────────────

class _EvidenceMetaCard extends StatelessWidget {
  const _EvidenceMetaCard({required this.pack});

  final EvidencePack pack;

  @override
  Widget build(BuildContext context) {
    return _EvidenceCard(
      title: 'Pack Metadata',
      icon: Icons.fingerprint,
      children: [
        _MetaRow('Run ID', pack.runId),
        _MetaRow('Worktree commit', _shortSha(pack.worktreeCommit)),
        _MetaRow('Instruction hash', _shortSha(pack.instructionHash)),
        _MetaRow('Policy hash', _shortSha(pack.policyHash)),
        _MetaRow('Created', pack.createdAt),
      ],
    );
  }

  static String _shortSha(String sha) =>
      sha.length > 12 ? sha.substring(0, 12) : sha;
}

// ─── Diff stats ───────────────────────────────────────────────────────────────

class _DiffStatsCard extends StatelessWidget {
  const _DiffStatsCard({required this.stats});

  final EvidenceDiffStats stats;

  @override
  Widget build(BuildContext context) {
    return _EvidenceCard(
      title: 'Diff Stats',
      icon: Icons.edit_document,
      children: [
        _StatRow(
          label: '${stats.filesChanged} file${stats.filesChanged == 1 ? '' : 's'} changed',
          color: Colors.white70,
        ),
        _StatRow(
          label: '+${stats.insertions} insertions',
          color: ClawdTheme.success,
        ),
        _StatRow(
          label: '−${stats.deletions} deletions',
          color: ClawdTheme.error,
        ),
        if (stats.files.isNotEmpty) ...[
          const SizedBox(height: 8),
          ...stats.files.take(8).map(
                (f) => Padding(
                  padding: const EdgeInsets.only(bottom: 3),
                  child: Row(
                    children: [
                      const Icon(Icons.insert_drive_file_outlined,
                          size: 11, color: Colors.white24),
                      const SizedBox(width: 5),
                      Expanded(
                        child: Text(
                          f,
                          style: const TextStyle(
                              fontSize: 10, color: Colors.white38),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
          if (stats.files.length > 8)
            Text(
              '+ ${stats.files.length - 8} more',
              style: const TextStyle(fontSize: 10, color: Colors.white24),
            ),
        ],
      ],
    );
  }
}

// ─── Test results ─────────────────────────────────────────────────────────────

class _TestResultsCard extends StatelessWidget {
  const _TestResultsCard({required this.results});

  final EvidenceTestResults results;

  @override
  Widget build(BuildContext context) {
    return _EvidenceCard(
      title: 'Test Results',
      icon: Icons.science_outlined,
      children: [
        Row(
          children: [
            _TestBadge(
                count: results.passed, label: 'passed', color: ClawdTheme.success),
            const SizedBox(width: 8),
            if (results.failed > 0)
              _TestBadge(
                  count: results.failed, label: 'failed', color: ClawdTheme.error),
            if (results.skipped > 0) ...[
              const SizedBox(width: 8),
              _TestBadge(
                  count: results.skipped, label: 'skipped', color: Colors.white38),
            ],
            const Spacer(),
            Text(
              '${results.durationMs}ms',
              style: const TextStyle(fontSize: 11, color: Colors.white38),
            ),
          ],
        ),
        if (results.firstFailure != null) ...[
          const SizedBox(height: 8),
          Container(
            padding:
                const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            decoration: BoxDecoration(
              color: ClawdTheme.error.withValues(alpha: 0.08),
              borderRadius: BorderRadius.circular(4),
            ),
            child: Row(
              children: [
                const Icon(Icons.close, size: 12, color: ClawdTheme.error),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(
                    results.firstFailure!,
                    style: const TextStyle(
                        fontSize: 11, color: ClawdTheme.error),
                  ),
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }
}

class _TestBadge extends StatelessWidget {
  const _TestBadge(
      {required this.count, required this.label, required this.color});

  final int count;
  final String label;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        '$count $label',
        style: TextStyle(fontSize: 11, color: color),
      ),
    );
  }
}

// ─── Tool trace ───────────────────────────────────────────────────────────────

class _ToolTraceCard extends StatelessWidget {
  const _ToolTraceCard({required this.trace});

  final List<EvidenceToolTrace> trace;

  @override
  Widget build(BuildContext context) {
    if (trace.isEmpty) {
      return const _EvidenceCard(
        title: 'Tool Trace',
        icon: Icons.terminal,
        children: [
          Text('No tool calls recorded.',
              style: TextStyle(fontSize: 12, color: Colors.white38)),
        ],
      );
    }

    return _EvidenceCard(
      title: 'Tool Trace (${trace.length})',
      icon: Icons.terminal,
      children: trace.take(20).map((t) => _ToolTraceRow(trace: t)).toList(),
    );
  }
}

class _ToolTraceRow extends StatelessWidget {
  const _ToolTraceRow({required this.trace});

  final EvidenceToolTrace trace;

  Color get _decisionColor {
    switch (trace.decision) {
      case 'blocked':
        return ClawdTheme.error;
      case 'warned':
        return ClawdTheme.warning;
      default:
        return ClawdTheme.success;
    }
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        children: [
          Container(
            width: 6,
            height: 6,
            margin: const EdgeInsets.only(right: 8, top: 4),
            decoration: BoxDecoration(
              color: _decisionColor,
              shape: BoxShape.circle,
            ),
          ),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(
                      trace.tool,
                      style: TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.w600,
                        color: _decisionColor,
                      ),
                    ),
                    const SizedBox(width: 6),
                    Text(
                      '${trace.durationMs}ms',
                      style: const TextStyle(
                          fontSize: 10, color: Colors.white24),
                    ),
                  ],
                ),
                Text(
                  trace.path,
                  style: const TextStyle(fontSize: 10, color: Colors.white38),
                  overflow: TextOverflow.ellipsis,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

// ─── Reviewer verdict ─────────────────────────────────────────────────────────

class _ReviewerVerdictCard extends StatelessWidget {
  const _ReviewerVerdictCard({required this.verdict});

  final String verdict;

  @override
  Widget build(BuildContext context) {
    final isApproved = verdict.toLowerCase().contains('approved') ||
        verdict.toLowerCase().contains('pass');
    final color =
        isApproved ? ClawdTheme.success : ClawdTheme.warning;

    return _EvidenceCard(
      title: 'Reviewer Verdict',
      icon: Icons.rate_review_outlined,
      children: [
        Row(
          children: [
            Icon(
              isApproved ? Icons.check_circle_outline : Icons.pending_outlined,
              size: 16,
              color: color,
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                verdict,
                style: TextStyle(fontSize: 13, color: color),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

// ─── Shared evidence card wrapper ─────────────────────────────────────────────

class _EvidenceCard extends StatelessWidget {
  const _EvidenceCard({
    required this.title,
    required this.icon,
    required this.children,
  });

  final String title;
  final IconData icon;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding:
                const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
            child: Row(
              children: [
                Icon(icon, size: 14, color: ClawdTheme.claw),
                const SizedBox(width: 8),
                Text(
                  title,
                  style: const TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                    color: Colors.white70,
                    letterSpacing: 0.5,
                  ),
                ),
              ],
            ),
          ),
          const Divider(
              height: 1, thickness: 1, color: ClawdTheme.surfaceBorder),
          Padding(
            padding: const EdgeInsets.all(14),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: children,
            ),
          ),
        ],
      ),
    );
  }
}

// ─── Small shared helpers ─────────────────────────────────────────────────────

class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(
      text.toUpperCase(),
      style: const TextStyle(
        fontSize: 10,
        fontWeight: FontWeight.w600,
        color: Colors.white38,
        letterSpacing: 1.0,
      ),
    );
  }
}

class _InfoChip extends StatelessWidget {
  const _InfoChip(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: ClawdTheme.claw.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(text,
          style:
              const TextStyle(fontSize: 11, color: ClawdTheme.clawLight)),
    );
  }
}

class _MetaRow extends StatelessWidget {
  const _MetaRow(this.label, this.value);

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 120,
            child: Text(
              label,
              style:
                  const TextStyle(fontSize: 11, color: Colors.white38),
            ),
          ),
          Expanded(
            child: Text(
              value.isEmpty ? '—' : value,
              style:
                  const TextStyle(fontSize: 11, color: Colors.white70),
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
    );
  }
}

class _StatRow extends StatelessWidget {
  const _StatRow({required this.label, required this.color});

  final String label;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Text(label, style: TextStyle(fontSize: 13, color: color)),
    );
  }
}

class _TimelineRow extends StatelessWidget {
  const _TimelineRow({required this.label, required this.ts});

  final String label;
  final String? ts;

  @override
  Widget build(BuildContext context) {
    if (ts == null) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        children: [
          SizedBox(
            width: 90,
            child: Text(
              label,
              style: const TextStyle(fontSize: 11, color: Colors.white38),
            ),
          ),
          Text(
            ts!,
            style: const TextStyle(fontSize: 11, color: Colors.white54),
          ),
        ],
      ),
    );
  }
}
