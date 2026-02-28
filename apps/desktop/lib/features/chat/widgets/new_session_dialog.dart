import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_selector/file_selector.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/services/snackbar_service.dart';

class NewSessionDialog extends ConsumerStatefulWidget {
  const NewSessionDialog({super.key});

  @override
  ConsumerState<NewSessionDialog> createState() => _NewSessionDialogState();
}

class _NewSessionDialogState extends ConsumerState<NewSessionDialog> {
  final _pathController = TextEditingController();
  ProviderType _selectedProvider = ProviderType.claude;
  bool _creating = false;

  @override
  void dispose() {
    _pathController.dispose();
    super.dispose();
  }

  Future<void> _pickFolder() async {
    final path = await getDirectoryPath();
    if (path != null) {
      _pathController.text = path;
      setState(() {});
    }
  }

  Future<void> _create() async {
    final path = _pathController.text.trim();
    if (path.isEmpty) return;

    setState(() => _creating = true);
    try {
      final session = await ref.read(sessionListProvider.notifier).create(
            repoPath: path,
            provider: _selectedProvider,
          );
      if (mounted) {
        ref.read(activeSessionIdProvider.notifier).state = session.id;
        Navigator.of(context).pop();
      }
    } catch (e) {
      SnackbarService.instance.showError('Failed to create session: $e');
    } finally {
      if (mounted) setState(() => _creating = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final canCreate = _pathController.text.trim().isNotEmpty && !_creating;

    return AlertDialog(
      title: const Text('New Session'),
      backgroundColor: ClawdTheme.surfaceElevated,
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Repository path field
            TextField(
              controller: _pathController,
              onChanged: (_) => setState(() {}),
              decoration: InputDecoration(
                labelText: 'Repository Path',
                hintText: '/path/to/your/repo',
                suffixIcon: IconButton(
                  icon: const Icon(Icons.folder_open, size: 18),
                  onPressed: _pickFolder,
                  tooltip: 'Browse…',
                ),
                border: const OutlineInputBorder(),
              ),
            ),
            const SizedBox(height: 20),
            const Text(
              'AI Provider',
              style: TextStyle(fontSize: 13, fontWeight: FontWeight.w600),
            ),
            const SizedBox(height: 8),
            // Provider selector
            _ProviderSelector(
              selected: _selectedProvider,
              onChanged: (p) => setState(() => _selectedProvider = p),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: canCreate ? _create : null,
          style: FilledButton.styleFrom(backgroundColor: ClawdTheme.claw),
          child: _creating
              ? const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(
                      strokeWidth: 2, color: Colors.white),
                )
              : const Text('Create'),
        ),
      ],
    );
  }
}

class _ProviderSelector extends StatelessWidget {
  const _ProviderSelector({
    required this.selected,
    required this.onChanged,
  });

  final ProviderType selected;
  final ValueChanged<ProviderType> onChanged;

  @override
  Widget build(BuildContext context) {
    return RadioGroup<ProviderType>(
      groupValue: selected,
      onChanged: (v) { if (v != null) onChanged(v); },
      child: Column(
        children: ProviderType.values.map((p) {
          final (name, desc, color) = switch (p) {
            ProviderType.claude => (
                'Claude',
                'Best for code generation and architecture',
                ClawdTheme.claudeColor
              ),
            ProviderType.codex => (
                'Codex',
                'Best for debugging and explanation',
                ClawdTheme.codexColor
              ),
            ProviderType.cursor => (
                'Cursor',
                'Best for navigation and search',
                ClawdTheme.cursorColor
              ),
          };
          return InkWell(
            onTap: () => onChanged(p),
            child: Padding(
              padding: const EdgeInsets.symmetric(vertical: 4),
              child: Row(
                children: [
                  Radio<ProviderType>(
                    value: p,
                    materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                  ),
                  ProviderBadge(provider: p),
                  const SizedBox(width: 8),
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(name,
                          style: TextStyle(
                              fontWeight: FontWeight.w600,
                              color: color,
                              fontSize: 13)),
                      Text(desc,
                          style: const TextStyle(
                              fontSize: 11, color: Colors.white54)),
                    ],
                  ),
                ],
              ),
            ),
          );
        }).toList(),
      ),
    );
  }
}
