import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

// ─── Provider ─────────────────────────────────────────────────────────────────

/// Sprint EE CI.4 — CI run state notifier.
///
/// State: { runId, status, steps }
class _CiState {
  const _CiState({
    this.runId,
    this.status = 'idle',
    this.steps = const [],
  });

  final String? runId;
  final String status; // idle | running | success | failure | canceled
  final List<Map<String, dynamic>> steps;

  _CiState copyWith({
    String? runId,
    String? status,
    List<Map<String, dynamic>>? steps,
  }) =>
      _CiState(
        runId: runId ?? this.runId,
        status: status ?? this.status,
        steps: steps ?? this.steps,
      );
}

class _CiNotifier extends StateNotifier<_CiState> {
  _CiNotifier(this._ref) : super(const _CiState());

  final Ref _ref;

  Future<void> startRun(String repoPath) async {
    state = state.copyWith(status: 'running', steps: []);
    try {
      final client = _ref.read(daemonProvider.notifier).client;
      final result = await client.call<Map<String, dynamic>>(
        'ci.run',
        {'repoPath': repoPath},
      );
      final runId = result['runId'] as String? ?? '';
      state = state.copyWith(runId: runId, status: 'running');
    } catch (e) {
      state = state.copyWith(status: 'failure');
    }
  }

  Future<void> cancelRun() async {
    final runId = state.runId;
    if (runId == null) return;
    try {
      final client = _ref.read(daemonProvider.notifier).client;
      await client.call<Map<String, dynamic>>(
        'ci.cancel',
        {'runId': runId},
      );
      state = state.copyWith(status: 'canceled');
    } catch (_) {}
  }

  void addStepResult(Map<String, dynamic> stepResult) {
    state = state.copyWith(steps: [...state.steps, stepResult]);
  }

  void setComplete(String status) {
    state = state.copyWith(status: status);
  }
}

final _ciProvider =
    StateNotifierProvider.autoDispose<_CiNotifier, _CiState>(
  (ref) => _CiNotifier(ref),
);

// ─── Widget ───────────────────────────────────────────────────────────────────

/// Sprint EE CI.4 — CI Panel.
///
/// Shows CI run steps, status, and run/cancel buttons.
/// Intended to be embedded in the session sidebar or as a standalone panel.
class CiPanel extends ConsumerWidget {
  const CiPanel({super.key, this.repoPath = '.'});

  final String repoPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(_ciProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _CiHeader(
          status: state.status,
          onRun: state.status == 'idle' || state.status == 'success' || state.status == 'failure'
              ? () => ref.read(_ciProvider.notifier).startRun(repoPath)
              : null,
          onCancel: state.status == 'running'
              ? () => ref.read(_ciProvider.notifier).cancelRun()
              : null,
        ),
        const SizedBox(height: 12),
        if (state.steps.isEmpty && state.status != 'idle')
          const Padding(
            padding: EdgeInsets.symmetric(horizontal: 16),
            child: LinearProgressIndicator(),
          )
        else if (state.steps.isNotEmpty)
          ..._buildStepList(state.steps),
        if (state.status == 'idle')
          const Padding(
            padding: EdgeInsets.all(16),
            child: Text(
              'No CI run yet. Click Run to execute .claw/ci.yaml.',
              style: TextStyle(fontSize: 12, color: Colors.white38),
            ),
          ),
      ],
    );
  }

  List<Widget> _buildStepList(List<Map<String, dynamic>> steps) {
    return steps.map((step) {
      final name = step['stepName'] as String? ?? '';
      final status = step['status'] as String? ?? '';
      final output = step['output'] as String? ?? '';
      final durationMs = step['durationMs'] as int? ?? 0;
      final succeeded = status == 'success';

      return ListTile(
        dense: true,
        leading: Icon(
          succeeded ? Icons.check_circle : Icons.error_outline,
          size: 18,
          color: succeeded ? Colors.green : Colors.redAccent,
        ),
        title: Text(name, style: const TextStyle(fontSize: 13)),
        subtitle: output.isNotEmpty
            ? Text(
                output.split('\n').first,
                style: const TextStyle(fontSize: 11, color: Colors.white38),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              )
            : null,
        trailing: Text(
          '${(durationMs / 1000).toStringAsFixed(1)}s',
          style: const TextStyle(fontSize: 11, color: Colors.white38),
        ),
      );
    }).toList();
  }
}

class _CiHeader extends StatelessWidget {
  const _CiHeader({
    required this.status,
    this.onRun,
    this.onCancel,
  });

  final String status;
  final VoidCallback? onRun;
  final VoidCallback? onCancel;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: Row(
        children: [
          const Icon(
            Icons.play_circle_outline,
            size: 16,
            color: ClawdTheme.clawLight,
          ),
          const SizedBox(width: 6),
          Text(
            'CI Runner',
            style: Theme.of(context)
                .textTheme
                .titleSmall
                ?.copyWith(color: Colors.white),
          ),
          const Spacer(),
          _StatusBadge(status: status),
          const SizedBox(width: 8),
          if (onRun != null)
            FilledButton(
              onPressed: onRun,
              style: FilledButton.styleFrom(
                backgroundColor: ClawdTheme.claw,
                padding: const EdgeInsets.symmetric(
                    horizontal: 12, vertical: 4),
                minimumSize: Size.zero,
                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
              ),
              child: const Text('Run', style: TextStyle(fontSize: 12)),
            ),
          if (onCancel != null)
            OutlinedButton(
              onPressed: onCancel,
              style: OutlinedButton.styleFrom(
                padding: const EdgeInsets.symmetric(
                    horizontal: 12, vertical: 4),
                minimumSize: Size.zero,
                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
              ),
              child: const Text('Cancel',
                  style: TextStyle(fontSize: 12, color: Colors.white54)),
            ),
        ],
      ),
    );
  }
}

class _StatusBadge extends StatelessWidget {
  const _StatusBadge({required this.status});

  final String status;

  @override
  Widget build(BuildContext context) {
    final (color, label) = switch (status) {
      'running' => (Colors.blue, 'Running'),
      'success' => (Colors.green, 'Passed'),
      'failure' => (Colors.redAccent, 'Failed'),
      'canceled' => (Colors.orange, 'Canceled'),
      _ => (Colors.white38, 'Idle'),
    };

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(4),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Text(
        label,
        style: TextStyle(fontSize: 10, color: color),
      ),
    );
  }
}
