import 'package:flutter/material.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Displays a git-status-aware file tree built from a flat list of [FileStatus].
class FileTree extends StatefulWidget {
  const FileTree({
    super.key,
    required this.files,
    required this.selectedFile,
    required this.onFileTap,
  });

  final List<FileStatus> files;
  final FileStatus? selectedFile;
  final ValueChanged<FileStatus> onFileTap;

  @override
  State<FileTree> createState() => _FileTreeState();
}

class _FileTreeState extends State<FileTree> {
  final Set<String> _expanded = {};

  Color _stateColor(FileState state) => switch (state) {
        FileState.clean => Colors.white24,
        FileState.untracked => Colors.white38,
        FileState.modified => Colors.amber,
        FileState.staged => Colors.green,
        FileState.deleted => Colors.red,
        FileState.conflict => Colors.orange,
      };

  String _stateLabel(FileState state) => switch (state) {
        FileState.clean => '',
        FileState.untracked => 'U',
        FileState.modified => 'M',
        FileState.staged => 'S',
        FileState.deleted => 'D',
        FileState.conflict => 'C',
      };

  /// Build a tree structure: { dirPath: { files, subdirs } }
  /// Returns a sorted list of root entries (files and dirs).
  Map<String, List<FileStatus>> _groupByDir(List<FileStatus> files) {
    final Map<String, List<FileStatus>> groups = {'': []};
    for (final f in files) {
      final normalized = f.path.replaceAll(r'\', '/');
      final slashIdx = normalized.lastIndexOf('/');
      final dir = slashIdx == -1 ? '' : normalized.substring(0, slashIdx);
      groups.putIfAbsent(dir, () => []).add(f);
    }
    return groups;
  }

  @override
  Widget build(BuildContext context) {
    final groups = _groupByDir(widget.files);
    return ListView(
      children: _buildEntries(groups, ''),
    );
  }

  List<Widget> _buildEntries(
      Map<String, List<FileStatus>> groups, String dir) {
    final widgets = <Widget>[];

    // Direct children files in this dir
    final files = groups[dir] ?? [];
    for (final file in files) {
      final normalized = file.path.replaceAll(r'\', '/');
      final name = normalized.split('/').last;
      final isSelected = widget.selectedFile?.path == file.path;
      widgets.add(_FileTile(
        name: name,
        file: file,
        isSelected: isSelected,
        stateColor: _stateColor(file.state),
        stateLabel: _stateLabel(file.state),
        onTap: () => widget.onFileTap(file),
        indent: _countSlashes(dir),
      ));
    }

    // Subdirectories
    final subdirs = groups.keys
        .where((k) => k.isNotEmpty && _isDirectChild(dir, k))
        .toList()
      ..sort();
    for (final subdir in subdirs) {
      final dirName = subdir.split('/').last;
      final isExpanded = _expanded.contains(subdir);
      widgets.add(_DirTile(
        name: dirName,
        isExpanded: isExpanded,
        indent: _countSlashes(dir),
        onTap: () => setState(() {
          if (isExpanded) {
            _expanded.remove(subdir);
          } else {
            _expanded.add(subdir);
          }
        }),
      ));
      if (isExpanded) {
        widgets.addAll(_buildEntries(groups, subdir));
      }
    }

    return widgets;
  }

  bool _isDirectChild(String parent, String candidate) {
    if (parent.isEmpty) {
      return !candidate.contains('/');
    }
    if (!candidate.startsWith('$parent/')) return false;
    final rest = candidate.substring(parent.length + 1);
    return !rest.contains('/');
  }

  int _countSlashes(String path) =>
      path.isEmpty ? 0 : path.split('/').length;
}

class _FileTile extends StatelessWidget {
  const _FileTile({
    required this.name,
    required this.file,
    required this.isSelected,
    required this.stateColor,
    required this.stateLabel,
    required this.onTap,
    required this.indent,
  });

  final String name;
  final FileStatus file;
  final bool isSelected;
  final Color stateColor;
  final String stateLabel;
  final VoidCallback onTap;
  final int indent;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: Container(
        height: 26,
        color: isSelected
            ? ClawdTheme.claw.withValues(alpha: 0.15)
            : Colors.transparent,
        padding: EdgeInsets.only(left: 12.0 + indent * 12),
        child: Row(
          children: [
            const Icon(Icons.insert_drive_file_outlined,
                size: 13, color: Colors.white38),
            const SizedBox(width: 6),
            Expanded(
              child: Text(
                name,
                style: const TextStyle(fontSize: 12, color: Colors.white70),
                overflow: TextOverflow.ellipsis,
              ),
            ),
            Container(
              width: 18,
              height: 14,
              alignment: Alignment.center,
              margin: const EdgeInsets.only(right: 8),
              decoration: BoxDecoration(
                color: stateColor.withValues(alpha: 0.2),
                borderRadius: BorderRadius.circular(3),
              ),
              child: Text(
                stateLabel,
                style: TextStyle(
                  fontSize: 10,
                  fontWeight: FontWeight.w700,
                  color: stateColor,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _DirTile extends StatelessWidget {
  const _DirTile({
    required this.name,
    required this.isExpanded,
    required this.indent,
    required this.onTap,
  });

  final String name;
  final bool isExpanded;
  final int indent;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: SizedBox(
        height: 26,
        child: Padding(
          padding: EdgeInsets.only(left: 8.0 + indent * 12),
          child: Row(
            children: [
              Icon(
                isExpanded ? Icons.expand_more : Icons.chevron_right,
                size: 14,
                color: Colors.white38,
              ),
              const SizedBox(width: 4),
              Icon(
                isExpanded ? Icons.folder_open : Icons.folder,
                size: 13,
                color: Colors.amber.withValues(alpha: 0.8),
              ),
              const SizedBox(width: 6),
              Text(
                name,
                style: const TextStyle(fontSize: 12, color: Colors.white70),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
