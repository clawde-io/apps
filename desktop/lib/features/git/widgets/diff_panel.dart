import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/features/repo/repo_context_provider.dart';
import 'package:clawde/features/files/widgets/diff_view.dart';

/// Right pane of the Git screen — loads and renders a unified diff.
class DiffPanel extends ConsumerStatefulWidget {
  const DiffPanel({super.key, required this.selectedFile});
  final FileStatus? selectedFile;

  @override
  ConsumerState<DiffPanel> createState() => _DiffPanelState();
}

class _DiffPanelState extends ConsumerState<DiffPanel> {
  /// When true, show staged diff; when false, show unstaged diff.
  bool _showStaged = false;

  @override
  void didUpdateWidget(DiffPanel old) {
    super.didUpdateWidget(old);
    // Reset staged toggle when file selection changes.
    if (old.selectedFile?.path != widget.selectedFile?.path) {
      _showStaged = widget.selectedFile?.state == FileState.staged;
    }
  }

  @override
  Widget build(BuildContext context) {
    final file = widget.selectedFile;

    if (file == null) {
      return const EmptyState(
        icon: Icons.compare,
        title: 'Select a file',
        subtitle: 'Click a file in the changes list to view its diff',
      );
    }

    final hasBothViews = file.state == FileState.staged;
    final effectiveStaged = hasBothViews ? _showStaged : file.state == FileState.staged;
    final key = (file.path, effectiveStaged);
    final diffAsync = ref.watch(_diffProvider(key));

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // ── Header ─────────────────────────────────────────────────────────
        Container(
          height: 40,
          padding: const EdgeInsets.symmetric(horizontal: 16),
          decoration: const BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            border: Border(
              bottom: BorderSide(color: ClawdTheme.surfaceBorder),
            ),
          ),
          child: Row(
            children: [
              const Icon(Icons.compare, size: 14, color: Colors.white54),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  file.path,
                  style: const TextStyle(fontSize: 12, color: Colors.white70),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              // Staged / Unstaged toggle (only relevant for staged files)
              if (hasBothViews) ...[
                const SizedBox(width: 8),
                SegmentedButton<bool>(
                  segments: const [
                    ButtonSegment(value: false, label: Text('Unstaged')),
                    ButtonSegment(value: true, label: Text('Staged')),
                  ],
                  selected: {_showStaged},
                  onSelectionChanged: (s) =>
                      setState(() => _showStaged = s.first),
                  style: SegmentedButton.styleFrom(
                    textStyle: const TextStyle(fontSize: 11),
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                  ),
                ),
              ],
            ],
          ),
        ),

        // ── Diff content ────────────────────────────────────────────────────
        Expanded(
          child: diffAsync.when(
            loading: () => const Center(child: CircularProgressIndicator()),
            error: (e, _) => ErrorState(
              icon: Icons.error_outline,
              title: 'Failed to load diff',
              description: e.toString(),
              onRetry: () => ref.invalidate(_diffProvider(key)),
            ),
            data: (diff) => diff.isEmpty
                ? const EmptyState(
                    icon: Icons.check_circle_outline,
                    title: 'No changes',
                    subtitle: 'This file has no diff to show',
                  )
                : DiffView(diff: diff),
          ),
        ),
      ],
    );
  }
}

// Key: (filePath, isStaged)
final _diffProvider =
    FutureProvider.family<String, (String, bool)>((ref, key) async {
  final (filePath, staged) = key;
  final client = ref.read(daemonProvider.notifier).client;
  final path = ref.read(effectiveRepoPathProvider);
  if (path == null) return '';
  return await client.call<String>('repo.fileDiff', {
    'repoPath': path,
    'path': filePath,
    'staged': staged,
  });
});
