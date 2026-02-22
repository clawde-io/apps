import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/features/repo/repo_context_provider.dart';
import 'package:clawde/features/files/widgets/diff_view.dart';

/// Right pane of the Files screen — shows a diff for the selected file.
class FileViewer extends ConsumerWidget {
  const FileViewer({super.key, required this.selectedFile});
  final FileStatus? selectedFile;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (selectedFile == null) {
      return const EmptyState(
        icon: Icons.code,
        title: 'Select a file',
        subtitle: 'Click a modified file in the tree to view its diff',
      );
    }

    final file = selectedFile!;
    final canShowDiff = file.state == FileState.modified ||
        file.state == FileState.staged ||
        file.state == FileState.conflicted;

    if (!canShowDiff) {
      return EmptyState(
        icon: Icons.info_outline,
        title: file.path.split('/').last,
        subtitle: 'File preview for ${file.state.name} files '
            'coming in a future version',
      );
    }

    return _DiffLoader(file: file);
  }
}

class _DiffLoader extends ConsumerWidget {
  const _DiffLoader({required this.file});
  final FileStatus file;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final key = (file.path, file.state == FileState.staged);
    final diffAsync = ref.watch(_fileDiffProvider(key));

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // Header
        Container(
          height: 36,
          padding: const EdgeInsets.symmetric(horizontal: 16),
          decoration: const BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            border: Border(bottom: BorderSide(color: ClawdTheme.surfaceBorder)),
          ),
          child: Row(
            children: [
              Icon(
                Icons.compare,
                size: 14,
                color: _stateColor(file.state),
              ),
              const SizedBox(width: 8),
              Text(
                file.path,
                style: const TextStyle(fontSize: 12, color: Colors.white70),
              ),
              const SizedBox(width: 8),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
                decoration: BoxDecoration(
                  color: _stateColor(file.state).withValues(alpha: 0.2),
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  file.state.name,
                  style: TextStyle(
                    fontSize: 10,
                    fontWeight: FontWeight.w700,
                    color: _stateColor(file.state),
                  ),
                ),
              ),
            ],
          ),
        ),
        // Diff content
        Expanded(
          child: diffAsync.when(
            loading: () => const Center(child: CircularProgressIndicator()),
            error: (e, _) => ErrorState(
              icon: Icons.error_outline,
              title: 'Failed to load diff',
              description: e.toString(),
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

  Color _stateColor(FileState state) => switch (state) {
        FileState.modified => Colors.amber,
        FileState.staged => Colors.green,
        FileState.conflicted => Colors.orange,
        _ => Colors.white38,
      };
}

// Key: (filePath, isStaged)
final _fileDiffProvider =
    FutureProvider.family<String, (String, bool)>((ref, key) async {
  final (filePath, staged) = key;
  final client = ref.read(daemonProvider.notifier).client;
  final path = ref.read(effectiveRepoPathProvider);
  if (path == null) return '';
  return await client.call<String>('repo.fileDiff', {
    'path': path,
    'file': filePath,
    'staged': staged,
  });
});
