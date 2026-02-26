import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

// ─── Provider ─────────────────────────────────────────────────────────────────

final _gitQueryProvider =
    StateNotifierProvider.autoDispose<_GitQueryNotifier, _GitQueryState>(
  (ref) => _GitQueryNotifier(ref),
);

class _GitQueryState {
  const _GitQueryState({
    this.loading = false,
    this.narrative,
    this.commits = const [],
    this.error,
    this.lastQuestion = '',
  });

  final bool loading;
  final String? narrative;
  final List<Map<String, dynamic>> commits;
  final String? error;
  final String lastQuestion;

  _GitQueryState copyWith({
    bool? loading,
    String? narrative,
    List<Map<String, dynamic>>? commits,
    String? error,
    String? lastQuestion,
  }) =>
      _GitQueryState(
        loading: loading ?? this.loading,
        narrative: narrative ?? this.narrative,
        commits: commits ?? this.commits,
        error: error ?? this.error,
        lastQuestion: lastQuestion ?? this.lastQuestion,
      );
}

class _GitQueryNotifier extends StateNotifier<_GitQueryState> {
  _GitQueryNotifier(this._ref) : super(const _GitQueryState());

  final Ref _ref;

  Future<void> query(String question) async {
    state = state.copyWith(loading: true, error: null, lastQuestion: question);
    try {
      final client = _ref.read(daemonProvider.notifier).client;
      final result = await client.call<Map<String, dynamic>>('git.query', {
        'question': question,
        'repoPath': '.',
      });
      final commits = (result['commits'] as List?)
              ?.cast<Map<String, dynamic>>() ??
          [];
      state = state.copyWith(
        loading: false,
        narrative: result['narrative'] as String?,
        commits: commits,
      );
    } catch (e) {
      state = state.copyWith(loading: false, error: e.toString());
    }
  }
}

// ─── Screen ──────────────────────────────────────────────────────────────────

/// Sprint DD NL.5 — Natural Language Git History screen.
///
/// Users type questions like "What changed in auth last week?" and get
/// a structured commit list + AI narrative response.
class GitHistoryScreen extends ConsumerStatefulWidget {
  const GitHistoryScreen({super.key});

  @override
  ConsumerState<GitHistoryScreen> createState() => _GitHistoryScreenState();
}

class _GitHistoryScreenState extends ConsumerState<GitHistoryScreen> {
  final _ctrl = TextEditingController();

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  void _search() {
    final q = _ctrl.text.trim();
    if (q.isEmpty) return;
    ref.read(_gitQueryProvider.notifier).query(q);
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(_gitQueryProvider);

    return Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // ── Header ──────────────────────────────────────────────────────────
          const Text(
            'Git History',
            style: TextStyle(
                fontSize: 20,
                fontWeight: FontWeight.w700,
                color: Colors.white),
          ),
          const SizedBox(height: 4),
          const Text(
            'Ask a question about your commit history in plain English.',
            style: TextStyle(fontSize: 13, color: Colors.white54),
          ),
          const SizedBox(height: 20),

          // ── Search input ─────────────────────────────────────────────────
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _ctrl,
                  onSubmitted: (_) => _search(),
                  style: const TextStyle(fontSize: 14),
                  decoration: InputDecoration(
                    hintText: 'e.g. "What changed in auth last week?"',
                    prefixIcon:
                        const Icon(Icons.history, color: Colors.white38),
                    border: const OutlineInputBorder(),
                    contentPadding: const EdgeInsets.symmetric(
                        horizontal: 12, vertical: 10),
                    suffixIcon: state.loading
                        ? const Padding(
                            padding: EdgeInsets.all(12),
                            child: SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(
                                  strokeWidth: 2),
                            ),
                          )
                        : null,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              FilledButton(
                onPressed: state.loading ? null : _search,
                style: FilledButton.styleFrom(
                    backgroundColor: ClawdTheme.claw),
                child: const Text('Ask'),
              ),
            ],
          ),

          const SizedBox(height: 16),

          // ── Suggested queries ────────────────────────────────────────────
          if (state.narrative == null && !state.loading)
            Wrap(
              spacing: 8,
              children: [
                'What changed last week?',
                'Recent bug fixes',
                'Last 3 commits',
                'Changes in tests',
              ]
                  .map(
                    (q) => ActionChip(
                      label: Text(q,
                          style: const TextStyle(fontSize: 11)),
                      onPressed: () {
                        _ctrl.text = q;
                        ref
                            .read(_gitQueryProvider.notifier)
                            .query(q);
                      },
                    ),
                  )
                  .toList(),
            ),

          // ── Error ────────────────────────────────────────────────────────
          if (state.error != null)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text('Error: ${state.error}',
                  style: const TextStyle(color: Colors.redAccent)),
            ),

          const SizedBox(height: 16),

          // ── Results ──────────────────────────────────────────────────────
          if (state.narrative != null) ...[
            // Narrative card.
            Card(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        const Icon(Icons.auto_awesome,
                            size: 14, color: ClawdTheme.clawLight),
                        const SizedBox(width: 6),
                        Text(
                          'Summary',
                          style: Theme.of(context)
                              .textTheme
                              .labelMedium
                              ?.copyWith(color: ClawdTheme.clawLight),
                        ),
                      ],
                    ),
                    const SizedBox(height: 8),
                    Text(
                      state.narrative!,
                      style: const TextStyle(
                          fontSize: 13, height: 1.5),
                    ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 12),

            // Commit list.
            Expanded(
              child: state.commits.isEmpty
                  ? const Center(
                      child: Text('No commits found.',
                          style: TextStyle(color: Colors.white38)))
                  : ListView.separated(
                      itemCount: state.commits.length,
                      separatorBuilder: (_, __) =>
                          const Divider(height: 1),
                      itemBuilder: (context, i) {
                        final commit = state.commits[i];
                        return ListTile(
                          dense: true,
                          leading: Container(
                            padding: const EdgeInsets.symmetric(
                                horizontal: 8, vertical: 4),
                            decoration: BoxDecoration(
                              color: ClawdTheme.surfaceElevated,
                              borderRadius:
                                  BorderRadius.circular(4),
                            ),
                            child: Text(
                              commit['sha'] as String? ?? '',
                              style: const TextStyle(
                                fontSize: 11,
                                fontFamily: 'monospace',
                                color: ClawdTheme.clawLight,
                              ),
                            ),
                          ),
                          title: Text(
                            commit['subject'] as String? ?? '',
                            style: const TextStyle(fontSize: 13),
                          ),
                          subtitle: Text(
                            '${commit['author']} · ${commit['date']}',
                            style: const TextStyle(
                                fontSize: 11,
                                color: Colors.white38),
                          ),
                        );
                      },
                    ),
            ),
          ] else if (!state.loading)
            const Expanded(child: SizedBox.shrink()),
        ],
      ),
    );
  }
}
