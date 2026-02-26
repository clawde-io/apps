// SPDX-License-Identifier: MIT
// Editor keyboard shortcuts (Sprint HH, ED.13).

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// Keyboard shortcut actions for the code editor.
enum EditorAction {
  save,         // Cmd+S
  closeTab,     // Cmd+W
  prevTab,      // Cmd+Shift+[
  nextTab,      // Cmd+Shift+]
  splitPane,    // Cmd+\
  globalSearch, // Cmd+K
}

/// Registers global editor keyboard shortcuts.
///
/// Wrap the editor subtree with this widget:
/// ```dart
/// EditorShortcuts(onAction: _handleAction, child: editorWidget)
/// ```
class EditorShortcuts extends StatelessWidget {
  const EditorShortcuts({super.key, required this.onAction, required this.child});

  final ValueChanged<EditorAction> onAction;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Shortcuts(
      shortcuts: {
        // Cmd+S — save
        const SingleActivator(LogicalKeyboardKey.keyS, meta: true): const _EditorIntent(EditorAction.save),
        const SingleActivator(LogicalKeyboardKey.keyS, control: true): const _EditorIntent(EditorAction.save),

        // Cmd+W — close tab
        const SingleActivator(LogicalKeyboardKey.keyW, meta: true): const _EditorIntent(EditorAction.closeTab),
        const SingleActivator(LogicalKeyboardKey.keyW, control: true): const _EditorIntent(EditorAction.closeTab),

        // Cmd+Shift+[ — previous tab
        const SingleActivator(LogicalKeyboardKey.bracketLeft, meta: true, shift: true):
            const _EditorIntent(EditorAction.prevTab),
        const SingleActivator(LogicalKeyboardKey.bracketLeft, control: true, shift: true):
            const _EditorIntent(EditorAction.prevTab),

        // Cmd+Shift+] — next tab
        const SingleActivator(LogicalKeyboardKey.bracketRight, meta: true, shift: true):
            const _EditorIntent(EditorAction.nextTab),
        const SingleActivator(LogicalKeyboardKey.bracketRight, control: true, shift: true):
            const _EditorIntent(EditorAction.nextTab),

        // Cmd+\ — split pane
        const SingleActivator(LogicalKeyboardKey.backslash, meta: true): const _EditorIntent(EditorAction.splitPane),
        const SingleActivator(LogicalKeyboardKey.backslash, control: true): const _EditorIntent(EditorAction.splitPane),

        // Cmd+K — global search
        const SingleActivator(LogicalKeyboardKey.keyK, meta: true): const _EditorIntent(EditorAction.globalSearch),
        const SingleActivator(LogicalKeyboardKey.keyK, control: true): const _EditorIntent(EditorAction.globalSearch),
      },
      child: Actions(
        actions: {
          _EditorIntent: CallbackAction<_EditorIntent>(onInvoke: (intent) {
            onAction(intent.action);
            return null;
          }),
        },
        child: child,
      ),
    );
  }
}

class _EditorIntent extends Intent {
  const _EditorIntent(this.action);

  final EditorAction action;
}
