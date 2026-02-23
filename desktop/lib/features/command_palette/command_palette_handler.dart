import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawde/features/command_palette/command_palette.dart';

/// Wraps its [child] in a focus scope that listens for the command palette
/// keyboard shortcut (Cmd+Shift+P on macOS, Ctrl+Shift+P elsewhere) and opens
/// [CommandPaletteDialog] when it fires.
///
/// Place this high in the widget tree so the shortcut is captured regardless
/// of which widget currently holds focus.
class CommandPaletteHandler extends ConsumerWidget {
  const CommandPaletteHandler({
    super.key,
    required this.child,
  });

  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Focus(
      onKeyEvent: (node, event) {
        if (event is! KeyDownEvent) return KeyEventResult.ignored;

        final isMac = Theme.of(context).platform == TargetPlatform.macOS;
        final modifierHeld = isMac
            ? HardwareKeyboard.instance.isMetaPressed
            : HardwareKeyboard.instance.isControlPressed;
        final shiftHeld = HardwareKeyboard.instance.isShiftPressed;
        final isKeyP = event.logicalKey == LogicalKeyboardKey.keyP;

        if (modifierHeld && shiftHeld && isKeyP) {
          showCommandPalette(context, ref);
          return KeyEventResult.handled;
        }

        return KeyEventResult.ignored;
      },
      child: child,
    );
  }
}
