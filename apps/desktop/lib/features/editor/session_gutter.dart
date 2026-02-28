// SPDX-License-Identifier: MIT
// Session-aware gutter — annotates editor lines touched by AI sessions (Sprint HH, ED.9).

import 'package:flutter/material.dart';

/// A single line annotation from an AI session.
class SessionLineAnnotation {
  const SessionLineAnnotation({
    required this.line,
    required this.kind,
    required this.sessionId,
    this.sessionTitle,
  });

  /// 0-based line number.
  final int line;

  /// `"read"` | `"write"` | `"both"`
  final String kind;

  final String sessionId;
  final String? sessionTitle;

  Color get color {
    switch (kind) {
      case 'write':
        return const Color(0xFFdc2626); // brand red
      case 'read':
        return const Color(0xFF2563eb); // blue
      default:
        return const Color(0xFFd97706); // amber for "both"
    }
  }

  IconData get icon {
    switch (kind) {
      case 'write':
        return Icons.edit_outlined;
      case 'read':
        return Icons.visibility_outlined;
      default:
        return Icons.swap_horiz;
    }
  }
}

/// Displays a vertical strip of gutter annotations alongside a scrollable
/// line-height-spaced list.
///
/// Typically positioned to the left of the [EditorWebView] using a [Row].
class SessionGutter extends StatelessWidget {
  const SessionGutter({
    super.key,
    required this.annotations,
    required this.lineHeight,
    required this.scrollOffset,
    required this.visibleLines,
  });

  final List<SessionLineAnnotation> annotations;

  /// Height of a single line in logical pixels (match the editor font size).
  final double lineHeight;

  /// Current vertical scroll offset of the editor (in lines).
  final double scrollOffset;

  /// Number of lines visible in the viewport.
  final int visibleLines;

  @override
  Widget build(BuildContext context) {
    final firstLine = scrollOffset.floor();
    final lastLine = firstLine + visibleLines;

    final visible = annotations.where((a) => a.line >= firstLine && a.line <= lastLine).toList();

    return SizedBox(
      width: 16,
      child: Stack(
        children: visible
            .map(
              (a) => Positioned(
                top: (a.line - firstLine) * lineHeight,
                left: 0,
                right: 0,
                child: Tooltip(
                  message: a.sessionTitle != null
                      ? '${_kindLabel(a.kind)} by session "${a.sessionTitle}"'
                      : '${_kindLabel(a.kind)} by AI session',
                  child: Container(
                    height: lineHeight,
                    width: 4,
                    color: a.color.withValues(alpha: 0.7),
                  ),
                ),
              ),
            )
            .toList(),
      ),
    );
  }

  String _kindLabel(String kind) {
    switch (kind) {
      case 'write':
        return 'Written';
      case 'read':
        return 'Read';
      default:
        return 'Read and written';
    }
  }
}
