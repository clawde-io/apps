import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_selector/file_selector.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Dialog to create a new project with an optional root path.
class NewProjectDialog extends ConsumerStatefulWidget {
  const NewProjectDialog({super.key, this.onCreated});

  /// Called with the newly created [Project] after a successful create.
  final void Function(Project project)? onCreated;

  @override
  ConsumerState<NewProjectDialog> createState() => _NewProjectDialogState();
}

class _NewProjectDialogState extends ConsumerState<NewProjectDialog> {
  final _nameCtrl = TextEditingController();
  final _pathCtrl = TextEditingController();
  final _formKey = GlobalKey<FormState>();
  bool _loading = false;
  String? _error;

  @override
  void dispose() {
    _nameCtrl.dispose();
    _pathCtrl.dispose();
    super.dispose();
  }

  Future<void> _pickFolder() async {
    final result = await getDirectoryPath(
      confirmButtonText: 'Select Folder',
    );
    if (result != null && mounted) {
      setState(() => _pathCtrl.text = result);
      // Auto-fill name from the folder name if name is still empty.
      if (_nameCtrl.text.isEmpty) {
        final folderName = result.split('/').where((p) => p.isNotEmpty).lastOrNull ?? '';
        if (folderName.isNotEmpty) {
          _nameCtrl.text = folderName;
        }
      }
    }
  }

  Future<void> _submit() async {
    if (!(_formKey.currentState?.validate() ?? false)) return;
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final project = await ref.read(projectListProvider.notifier).create(
            name: _nameCtrl.text.trim(),
            rootPath: _pathCtrl.text.trim().isEmpty ? null : _pathCtrl.text.trim(),
          );
      if (mounted) {
        widget.onCreated?.call(project);
        Navigator.of(context).pop();
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _loading = false;
          _error = e.toString();
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      backgroundColor: ClawdTheme.surfaceElevated,
      title: const Text(
        'New Project',
        style: TextStyle(fontSize: 16, fontWeight: FontWeight.w700, color: Colors.white),
      ),
      content: SizedBox(
        width: 400,
        child: Form(
          key: _formKey,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Project name
              const Text(
                'Name',
                style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: Colors.white60),
              ),
              const SizedBox(height: 6),
              TextFormField(
                controller: _nameCtrl,
                autofocus: true,
                style: const TextStyle(fontSize: 13, color: Colors.white),
                decoration: const InputDecoration(
                  hintText: 'My Project',
                  border: OutlineInputBorder(),
                  contentPadding: EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                ),
                validator: (v) {
                  if (v == null || v.trim().isEmpty) return 'Name is required';
                  return null;
                },
                onFieldSubmitted: (_) => _submit(),
              ),
              const SizedBox(height: 16),

              // Root path (optional)
              const Text(
                'Root Path (optional)',
                style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: Colors.white60),
              ),
              const SizedBox(height: 6),
              Row(
                children: [
                  Expanded(
                    child: TextFormField(
                      controller: _pathCtrl,
                      style: const TextStyle(fontSize: 12, color: Colors.white70, fontFamily: 'monospace'),
                      decoration: const InputDecoration(
                        hintText: '/path/to/project',
                        border: OutlineInputBorder(),
                        contentPadding: EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  OutlinedButton.icon(
                    onPressed: _pickFolder,
                    icon: const Icon(Icons.folder_open, size: 14),
                    label: const Text('Browse'),
                    style: OutlinedButton.styleFrom(
                      foregroundColor: Colors.white60,
                      side: const BorderSide(color: ClawdTheme.surfaceBorder),
                      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 10),
                    ),
                  ),
                ],
              ),

              // Error message
              if (_error != null) ...[
                const SizedBox(height: 12),
                Text(
                  _error!,
                  style: const TextStyle(fontSize: 11, color: ClawdTheme.error),
                ),
              ],
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: _loading ? null : () => Navigator.of(context).pop(),
          style: TextButton.styleFrom(foregroundColor: Colors.white54),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: _loading ? null : _submit,
          style: FilledButton.styleFrom(
            backgroundColor: ClawdTheme.claw,
            foregroundColor: Colors.white,
          ),
          child: _loading
              ? const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white),
                )
              : const Text('Create'),
        ),
      ],
    );
  }
}
