import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/features/repo/repo_context_provider.dart';
import 'package:clawde/features/files/widgets/file_tree.dart';
import 'package:clawde/features/files/widgets/file_viewer.dart';

class FilesScreen extends ConsumerStatefulWidget {
  const FilesScreen({super.key});

  @override
  ConsumerState<FilesScreen> createState() => _FilesScreenState();
}

class _FilesScreenState extends ConsumerState<FilesScreen> {
  FileStatus? _selectedFile;

  @override
  Widget build(BuildContext context) {
    final pathAsync = ref.watch(effectiveRepoPathProvider);
    final repoAsync = ref.watch(activeRepoStatusProvider);

    if (pathAsync == null) {
      return const EmptyState(
        icon: Icons.folder_outlined,
        title: 'No active repository',
        subtitle: 'Select a session with an open repository',
      );
    }

    return Row(
      children: [
        // ── File tree (left pane) ────────────────────────────────────────
        SizedBox(
          width: 260,
          child: Container(
            decoration: const BoxDecoration(
              color: ClawdTheme.surfaceElevated,
              border: Border(
                right: BorderSide(color: ClawdTheme.surfaceBorder),
              ),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                // Tree header
                Container(
                  height: 40,
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: Row(
                    children: [
                      const Icon(Icons.folder_outlined,
                          size: 14, color: Colors.white54),
                      const SizedBox(width: 8),
                      const Expanded(
                        child: Text(
                          'Files',
                          style: TextStyle(
                              fontSize: 12,
                              fontWeight: FontWeight.w600,
                              color: Colors.white60),
                        ),
                      ),
                      repoAsync.when(
                        data: (_) => IconButton(
                          icon: const Icon(Icons.refresh,
                              size: 14, color: Colors.white38),
                          tooltip: 'Refresh',
                          onPressed: () =>
                              ref.invalidate(activeRepoStatusProvider),
                          padding: EdgeInsets.zero,
                          constraints: const BoxConstraints(),
                        ),
                        loading: () => const SizedBox(
                          width: 14,
                          height: 14,
                          child: CircularProgressIndicator(strokeWidth: 1.5),
                        ),
                        error: (_, __) => const SizedBox.shrink(),
                      ),
                    ],
                  ),
                ),
                const Divider(height: 1),
                // File list
                Expanded(
                  child: repoAsync.when(
                    loading: () =>
                        const Center(child: CircularProgressIndicator()),
                    error: (e, _) => ErrorState(
                      icon: Icons.error_outline,
                      title: 'Cannot load files',
                      description: e.toString(),
                      onRetry: () =>
                          ref.invalidate(activeRepoStatusProvider),
                    ),
                    data: (repo) {
                      if (repo == null || repo.files.isEmpty) {
                        return const EmptyState(
                          icon: Icons.check_circle_outline,
                          title: 'Working tree clean',
                          subtitle: 'No modified files',
                        );
                      }
                      return FileTree(
                        files: repo.files,
                        selectedFile: _selectedFile,
                        onFileTap: (f) =>
                            setState(() => _selectedFile = f),
                      );
                    },
                  ),
                ),
              ],
            ),
          ),
        ),

        // ── File viewer (right pane) ──────────────────────────────────────
        Expanded(
          child: FileViewer(selectedFile: _selectedFile),
        ),
      ],
    );
  }
}
