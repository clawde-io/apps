// SPDX-License-Identifier: MIT
/// Sprint ZZ IG.T08 — Flutter Instructions Panel.
///
/// Sidebar showing the instruction scope tree for the active project,
/// plus per-provider budget bars, a recompile button, and lint results.
library;

import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:clawde/features/repo/repo_context_provider.dart';

// ─── Instructions Panel ───────────────────────────────────────────────────────

/// Full-page instructions panel — scope tree, budget bars, lint results.
///
/// Wired into the sidebar navigation as `routeInstructions = '/instructions'`.
class InstructionsPanel extends ConsumerWidget {
  const InstructionsPanel({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final projectPath = ref.watch(effectiveRepoPathProvider);

    return Column(
      children: [
        _Header(projectPath: projectPath),
        Expanded(
          child: projectPath == null
              ? const _NoProjectPlaceholder()
              : _InstructionsBody(projectPath: projectPath),
        ),
      ],
    );
  }
}

// ─── Header ───────────────────────────────────────────────────────────────────

class _Header extends ConsumerWidget {
  const _Header({required this.projectPath});

  final String? projectPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      height: 56,
      padding: const EdgeInsets.symmetric(horizontal: 20),
      decoration: const BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        border: Border(
          bottom: BorderSide(color: ClawdTheme.surfaceBorder),
        ),
      ),
      child: Row(
        children: [
          const Icon(Icons.rule_outlined, size: 18, color: ClawdTheme.claw),
          const SizedBox(width: 10),
          const Text(
            'Instructions',
            style: TextStyle(
              fontSize: 15,
              fontWeight: FontWeight.w600,
              color: Colors.white,
            ),
          ),
          const Spacer(),
          if (projectPath != null)
            _CompileButton(projectPath: projectPath!),
        ],
      ),
    );
  }
}

// ─── Compile button ───────────────────────────────────────────────────────────

class _CompileButton extends ConsumerStatefulWidget {
  const _CompileButton({required this.projectPath});

  final String projectPath;

  @override
  ConsumerState<_CompileButton> createState() => _CompileButtonState();
}

class _CompileButtonState extends ConsumerState<_CompileButton> {
  bool _compiling = false;

  Future<void> _compile() async {
    setState(() => _compiling = true);
    try {
      final client = ref.read(daemonProvider.notifier).client;
      await client.instructionsCompile(projectPath: widget.projectPath);
      // Reload scope + budget after compile
      await ref
          .read(instructionScopeProvider(widget.projectPath).notifier)
          .load();
      await ref
          .read(instructionBudgetProvider(widget.projectPath).notifier)
          .load();
      await ref
          .read(instructionLintProvider(widget.projectPath).notifier)
          .load();
    } catch (_) {
      // Errors shown in section widgets
    } finally {
      if (mounted) setState(() => _compiling = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return TextButton.icon(
      onPressed: _compiling ? null : _compile,
      icon: _compiling
          ? const SizedBox(
              width: 14,
              height: 14,
              child: CircularProgressIndicator(
                  strokeWidth: 2, color: ClawdTheme.claw),
            )
          : const Icon(Icons.refresh, size: 16),
      label: Text(
        _compiling ? 'Compiling…' : 'Recompile',
        style: const TextStyle(fontSize: 12),
      ),
      style: TextButton.styleFrom(foregroundColor: ClawdTheme.claw),
    );
  }
}

// ─── Body ─────────────────────────────────────────────────────────────────────

class _InstructionsBody extends ConsumerStatefulWidget {
  const _InstructionsBody({required this.projectPath});

  final String projectPath;

  @override
  ConsumerState<_InstructionsBody> createState() => _InstructionsBodyState();
}

class _InstructionsBodyState extends ConsumerState<_InstructionsBody> {
  @override
  void initState() {
    super.initState();
    // Load on first mount
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref
          .read(instructionScopeProvider(widget.projectPath).notifier)
          .load();
      ref
          .read(instructionBudgetProvider(widget.projectPath).notifier)
          .load();
      ref
          .read(instructionLintProvider(widget.projectPath).notifier)
          .load();
    });
  }

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.symmetric(vertical: 12),
      children: [
        _BudgetSection(projectPath: widget.projectPath),
        const SizedBox(height: 8),
        _ScopeTreeSection(projectPath: widget.projectPath),
        const SizedBox(height: 8),
        _LintSection(projectPath: widget.projectPath),
      ],
    );
  }
}

// ─── Budget section ───────────────────────────────────────────────────────────

class _BudgetSection extends ConsumerWidget {
  const _BudgetSection({required this.projectPath});

  final String projectPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(instructionBudgetProvider(projectPath));

    return _Card(
      title: 'Budget',
      icon: Icons.pie_chart_outline,
      child: async.when(
        loading: () => const _LoadingRow(),
        error: (e, _) => _ErrorRow(message: e.toString()),
        data: (report) {
          if (report == null) {
            return const _EmptyRow('No budget data yet. Tap Recompile.');
          }
          return Column(
            children: [
              _BudgetBar(
                label: 'Claude',
                budget: report.claude,
                barColor: ClawdTheme.claudeColor,
              ),
              const SizedBox(height: 10),
              _BudgetBar(
                label: 'Codex',
                budget: report.codex,
                barColor: ClawdTheme.codexColor,
              ),
            ],
          );
        },
      ),
    );
  }
}

class _BudgetBar extends StatelessWidget {
  const _BudgetBar({
    required this.label,
    required this.budget,
    required this.barColor,
  });

  final String label;
  final InstructionBudget budget;
  final Color barColor;

  @override
  Widget build(BuildContext context) {
    final pct = budget.pct.clamp(0, 100);
    final color = budget.overBudget
        ? ClawdTheme.error
        : budget.nearBudget
            ? ClawdTheme.warning
            : barColor;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Text(label,
                style: const TextStyle(fontSize: 12, color: Colors.white70)),
            const Spacer(),
            Text(
              '${_fmtBytes(budget.bytesUsed)} / ${_fmtBytes(budget.budgetBytes)}',
              style: TextStyle(
                fontSize: 11,
                color: budget.overBudget ? ClawdTheme.error : Colors.white38,
              ),
            ),
            const SizedBox(width: 6),
            Text(
              '${budget.pct}%',
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w600,
                color: color,
              ),
            ),
          ],
        ),
        const SizedBox(height: 4),
        ClipRRect(
          borderRadius: BorderRadius.circular(2),
          child: LinearProgressIndicator(
            value: pct / 100,
            minHeight: 4,
            backgroundColor: ClawdTheme.surfaceBorder,
            valueColor: AlwaysStoppedAnimation<Color>(color),
          ),
        ),
      ],
    );
  }

  static String _fmtBytes(int bytes) {
    if (bytes < 1024) return '${bytes}B';
    return '${(bytes / 1024).toStringAsFixed(1)}KB';
  }
}

// ─── Scope tree section ───────────────────────────────────────────────────────

class _ScopeTreeSection extends ConsumerWidget {
  const _ScopeTreeSection({required this.projectPath});

  final String projectPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(instructionScopeProvider(projectPath));

    return _Card(
      title: 'Scope Tree',
      icon: Icons.account_tree_outlined,
      child: async.when(
        loading: () => const _LoadingRow(),
        error: (e, _) => _ErrorRow(message: e.toString()),
        data: (result) {
          if (result == null || result.nodes.isEmpty) {
            return const _EmptyRow('No instruction nodes found.');
          }
          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              ...result.nodes.map((n) => _NodeTile(node: n)),
              if (result.conflicts.isNotEmpty) ...[
                const SizedBox(height: 8),
                ...result.conflicts.map(
                  (c) => Padding(
                    padding: const EdgeInsets.only(bottom: 4),
                    child: Row(
                      children: [
                        const Icon(Icons.warning_amber_rounded,
                            size: 14, color: ClawdTheme.warning),
                        const SizedBox(width: 6),
                        Expanded(
                          child: Text(
                            c,
                            style: const TextStyle(
                                fontSize: 11, color: Colors.white54),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ],
          );
        },
      ),
    );
  }
}

class _NodeTile extends StatelessWidget {
  const _NodeTile({required this.node});

  final InstructionNode node;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            width: 32,
            height: 32,
            margin: const EdgeInsets.only(right: 10),
            decoration: BoxDecoration(
              color: ClawdTheme.claw.withValues(alpha: 0.12),
              borderRadius: BorderRadius.circular(6),
            ),
            child: const Icon(Icons.description_outlined,
                size: 16, color: ClawdTheme.claw),
          ),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        node.scope,
                        style: const TextStyle(
                          fontSize: 12,
                          fontWeight: FontWeight.w600,
                          color: Colors.white,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    _OwnerBadge(owner: node.owner),
                  ],
                ),
                const SizedBox(height: 2),
                Text(
                  node.preview.isEmpty ? '(empty)' : node.preview,
                  style: const TextStyle(fontSize: 11, color: Colors.white38),
                  maxLines: 2,
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

class _OwnerBadge extends StatelessWidget {
  const _OwnerBadge({required this.owner});

  final String owner;

  Color get _color {
    switch (owner) {
      case 'claude':
        return ClawdTheme.claudeColor;
      case 'codex':
        return ClawdTheme.codexColor;
      default:
        return ClawdTheme.info;
    }
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: _color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        owner,
        style: TextStyle(fontSize: 10, color: _color),
      ),
    );
  }
}

// ─── Lint section ─────────────────────────────────────────────────────────────

class _LintSection extends ConsumerWidget {
  const _LintSection({required this.projectPath});

  final String projectPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(instructionLintProvider(projectPath));

    return _Card(
      title: 'Lint',
      icon: Icons.checklist_rtl_outlined,
      child: async.when(
        loading: () => const _LoadingRow(),
        error: (e, _) => _ErrorRow(message: e.toString()),
        data: (report) {
          if (report == null) {
            return const _EmptyRow('Run lint to check instruction quality.');
          }
          if (report.passed) {
            return const Row(
              children: [
                Icon(Icons.check_circle_outline,
                    size: 16, color: ClawdTheme.success),
                SizedBox(width: 8),
                Text('All checks passed',
                    style: TextStyle(fontSize: 13, color: ClawdTheme.success)),
              ],
            );
          }
          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _LintSummaryRow(errors: report.errors, warnings: report.warnings),
              const SizedBox(height: 8),
              ...report.issues.map((i) => _LintIssueTile(issue: i)),
            ],
          );
        },
      ),
    );
  }
}

class _LintSummaryRow extends StatelessWidget {
  const _LintSummaryRow({required this.errors, required this.warnings});

  final int errors;
  final int warnings;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        if (errors > 0) ...[
          const Icon(Icons.cancel_outlined, size: 14, color: ClawdTheme.error),
          const SizedBox(width: 4),
          Text('$errors error${errors == 1 ? '' : 's'}',
              style: const TextStyle(fontSize: 12, color: ClawdTheme.error)),
          const SizedBox(width: 12),
        ],
        if (warnings > 0) ...[
          const Icon(Icons.warning_amber_rounded,
              size: 14, color: ClawdTheme.warning),
          const SizedBox(width: 4),
          Text('$warnings warning${warnings == 1 ? '' : 's'}',
              style: const TextStyle(fontSize: 12, color: ClawdTheme.warning)),
        ],
      ],
    );
  }
}

class _LintIssueTile extends StatelessWidget {
  const _LintIssueTile({required this.issue});

  final InstructionLintIssue issue;

  @override
  Widget build(BuildContext context) {
    final color =
        issue.isError ? ClawdTheme.error : ClawdTheme.warning;
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(
            issue.isError
                ? Icons.cancel_outlined
                : Icons.warning_amber_rounded,
            size: 14,
            color: color,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  issue.message,
                  style: TextStyle(fontSize: 12, color: color),
                ),
                if (issue.rule.isNotEmpty)
                  Text(
                    issue.rule,
                    style: const TextStyle(
                        fontSize: 10, color: Colors.white38),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

// ─── Shared card wrapper ──────────────────────────────────────────────────────

class _Card extends StatelessWidget {
  const _Card({
    required this.title,
    required this.icon,
    required this.child,
  });

  final String title;
  final IconData icon;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 16),
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
            child: child,
          ),
        ],
      ),
    );
  }
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

class _LoadingRow extends StatelessWidget {
  const _LoadingRow();

  @override
  Widget build(BuildContext context) {
    return const SizedBox(
      height: 24,
      child: Center(
        child: SizedBox(
          width: 16,
          height: 16,
          child:
              CircularProgressIndicator(strokeWidth: 2, color: ClawdTheme.claw),
        ),
      ),
    );
  }
}

class _ErrorRow extends StatelessWidget {
  const _ErrorRow({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        const Icon(Icons.error_outline, size: 14, color: ClawdTheme.error),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            message,
            style: const TextStyle(fontSize: 12, color: ClawdTheme.error),
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
          ),
        ),
      ],
    );
  }
}

class _EmptyRow extends StatelessWidget {
  const _EmptyRow(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(
      text,
      style: const TextStyle(fontSize: 12, color: Colors.white38),
    );
  }
}

class _NoProjectPlaceholder extends StatelessWidget {
  const _NoProjectPlaceholder();

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.folder_off_outlined, size: 40, color: Colors.white24),
          SizedBox(height: 12),
          Text(
            'Open a project to view instructions',
            style: TextStyle(fontSize: 13, color: Colors.white38),
          ),
        ],
      ),
    );
  }
}
