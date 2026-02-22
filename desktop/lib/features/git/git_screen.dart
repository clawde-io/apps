import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/features/repo/repo_context_provider.dart';
import 'package:clawde/features/git/widgets/diff_panel.dart';

class GitScreen extends ConsumerStatefulWidget {
  const GitScreen({super.key});

  @override
  ConsumerState<GitScreen> createState() => _GitScreenState();
}

class _GitScreenState extends ConsumerState<GitScreen> {
  FileStatus? _selectedFile;

  @override
  Widget build(BuildContext context) {
    final pathAsync = ref.watch(effectiveRepoPathProvider);
    final repoAsync = ref.watch(activeRepoStatusProvider);

    if (pathAsync == null) {
      return const EmptyState(
        icon: Icons.account_tree_outlined,
        title: 'No active repository',
        subtitle: 'Select a session with an open repository',
      );
    }

    return Row(
      children: [
        // ── Status overview (left) ───────────────────────────────────────
        SizedBox(
          width: 280,
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
                // Header
                Container(
                  height: 40,
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: Row(
                    children: [
                      const Icon(Icons.account_tree_outlined,
                          size: 14, color: Colors.white54),
                      const SizedBox(width: 8),
                      const Expanded(
                        child: Text(
                          'Changes',
                          style: TextStyle(
                            fontSize: 12,
                            fontWeight: FontWeight.w600,
                            color: Colors.white60,
                          ),
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
                Expanded(
                  child: repoAsync.when(
                    loading: () =>
                        const Center(child: CircularProgressIndicator()),
                    error: (e, _) => ErrorState(
                      icon: Icons.error_outline,
                      title: 'Cannot load repo status',
                      description: e.toString(),
                      onRetry: () =>
                          ref.invalidate(activeRepoStatusProvider),
                    ),
                    data: (repo) {
                      if (repo == null || repo.files.isEmpty) {
                        return const EmptyState(
                          icon: Icons.check_circle_outline,
                          title: 'Working tree clean',
                          subtitle: 'No uncommitted changes',
                        );
                      }
                      return _StatusOverview(
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

        // ── Diff panel (right) ────────────────────────────────────────────
        Expanded(
          child: DiffPanel(selectedFile: _selectedFile),
        ),
      ],
    );
  }
}

// ── Status overview ───────────────────────────────────────────────────────────

class _StatusOverview extends StatelessWidget {
  const _StatusOverview({
    required this.files,
    required this.selectedFile,
    required this.onFileTap,
  });

  final List<FileStatus> files;
  final FileStatus? selectedFile;
  final ValueChanged<FileStatus> onFileTap;

  List<FileStatus> _byState(FileState state) =>
      files.where((f) => f.state == state).toList();

  @override
  Widget build(BuildContext context) {
    final staged = _byState(FileState.staged);
    final modified = _byState(FileState.modified);
    final untracked = _byState(FileState.untracked);
    final deleted = _byState(FileState.deleted);
    final conflicted = _byState(FileState.conflicted);

    return ListView(
      children: [
        if (staged.isNotEmpty)
          _Section(
            title: 'Staged Changes',
            count: staged.length,
            color: Colors.green,
            files: staged,
            selectedFile: selectedFile,
            onFileTap: onFileTap,
          ),
        if (modified.isNotEmpty)
          _Section(
            title: 'Unstaged Changes',
            count: modified.length,
            color: Colors.amber,
            files: modified,
            selectedFile: selectedFile,
            onFileTap: onFileTap,
          ),
        if (untracked.isNotEmpty)
          _Section(
            title: 'Untracked Files',
            count: untracked.length,
            color: Colors.white38,
            files: untracked,
            selectedFile: selectedFile,
            onFileTap: onFileTap,
          ),
        if (deleted.isNotEmpty)
          _Section(
            title: 'Deleted Files',
            count: deleted.length,
            color: Colors.red,
            files: deleted,
            selectedFile: selectedFile,
            onFileTap: onFileTap,
          ),
        if (conflicted.isNotEmpty)
          _Section(
            title: 'Conflicts',
            count: conflicted.length,
            color: Colors.orange,
            files: conflicted,
            selectedFile: selectedFile,
            onFileTap: onFileTap,
          ),
      ],
    );
  }
}

class _Section extends StatefulWidget {
  const _Section({
    required this.title,
    required this.count,
    required this.color,
    required this.files,
    required this.selectedFile,
    required this.onFileTap,
  });

  final String title;
  final int count;
  final Color color;
  final List<FileStatus> files;
  final FileStatus? selectedFile;
  final ValueChanged<FileStatus> onFileTap;

  @override
  State<_Section> createState() => _SectionState();
}

class _SectionState extends State<_Section> {
  bool _expanded = true;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // Section header
        InkWell(
          onTap: () => setState(() => _expanded = !_expanded),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
            child: Row(
              children: [
                Icon(
                  _expanded ? Icons.expand_more : Icons.chevron_right,
                  size: 14,
                  color: Colors.white38,
                ),
                const SizedBox(width: 4),
                Text(
                  widget.title,
                  style: const TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                    color: Colors.white54,
                  ),
                ),
                const SizedBox(width: 6),
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                  decoration: BoxDecoration(
                    color: widget.color.withValues(alpha: 0.2),
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Text(
                    '${widget.count}',
                    style: TextStyle(
                      fontSize: 10,
                      fontWeight: FontWeight.w700,
                      color: widget.color,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
        if (_expanded)
          ...widget.files.map(
            (f) => _FileLine(
              file: f,
              color: widget.color,
              isSelected: widget.selectedFile?.path == f.path,
              onTap: () => widget.onFileTap(f),
            ),
          ),
      ],
    );
  }
}

class _FileLine extends StatelessWidget {
  const _FileLine({
    required this.file,
    required this.color,
    required this.isSelected,
    required this.onTap,
  });

  final FileStatus file;
  final Color color;
  final bool isSelected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final name = file.path.split('/').last;
    return InkWell(
      onTap: onTap,
      child: Container(
        height: 24,
        color: isSelected
            ? ClawdTheme.claw.withValues(alpha: 0.15)
            : Colors.transparent,
        padding: const EdgeInsets.symmetric(horizontal: 28),
        child: Row(
          children: [
            Container(
              width: 6,
              height: 6,
              decoration:
                  BoxDecoration(color: color, shape: BoxShape.circle),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                name,
                style: const TextStyle(fontSize: 12, color: Colors.white70),
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
