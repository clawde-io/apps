// SPDX-License-Identifier: MIT
// Split-pane editor view (Sprint HH, ED.5).

import 'package:flutter/material.dart';

import 'package:clawde/features/editor/editor_webview.dart';
import 'package:clawde/features/editor/js_bridge.dart';

/// Split direction for [SplitEditorView].
enum SplitDirection { horizontal, vertical }

/// Two-pane split editor view.
///
/// Each pane is an independent [EditorWebView] instance.  Drag the divider
/// to resize.  Use [primaryBridge] / [secondaryBridge] to send open/patch
/// commands to the appropriate pane.
class SplitEditorView extends StatefulWidget {
  const SplitEditorView({
    super.key,
    this.direction = SplitDirection.horizontal,
    this.initialRatio = 0.5,
    this.onPrimaryReady,
    this.onSecondaryReady,
    this.onPrimaryEvent,
    this.onSecondaryEvent,
  });

  final SplitDirection direction;

  /// Initial split ratio: 0.0 (full primary) to 1.0 (full secondary).
  final double initialRatio;

  final ValueChanged<JsBridge>? onPrimaryReady;
  final ValueChanged<JsBridge>? onSecondaryReady;
  final ValueChanged<EditorEvent>? onPrimaryEvent;
  final ValueChanged<EditorEvent>? onSecondaryEvent;

  @override
  State<SplitEditorView> createState() => _SplitEditorViewState();
}

class _SplitEditorViewState extends State<SplitEditorView> {
  late double _ratio;

  static const _dividerThickness = 4.0;
  static const _minPaneRatio = 0.15;

  @override
  void initState() {
    super.initState();
    _ratio = widget.initialRatio;
  }

  void _onDrag(DragUpdateDetails details, BoxConstraints constraints) {
    final totalSize = widget.direction == SplitDirection.horizontal
        ? constraints.maxWidth - _dividerThickness
        : constraints.maxHeight - _dividerThickness;

    final delta = widget.direction == SplitDirection.horizontal
        ? details.delta.dx
        : details.delta.dy;

    setState(() {
      _ratio = (_ratio + delta / totalSize).clamp(_minPaneRatio, 1.0 - _minPaneRatio);
    });
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final isH = widget.direction == SplitDirection.horizontal;
        final totalSize = isH ? constraints.maxWidth : constraints.maxHeight;
        final primarySize = (totalSize - _dividerThickness) * _ratio;
        final secondarySize = (totalSize - _dividerThickness) * (1.0 - _ratio);

        final primary = SizedBox(
          width: isH ? primarySize : constraints.maxWidth,
          height: isH ? constraints.maxHeight : primarySize,
          child: EditorWebView(onReady: widget.onPrimaryReady, onEvent: widget.onPrimaryEvent),
        );

        final divider = GestureDetector(
          onHorizontalDragUpdate: isH ? (d) => _onDrag(d, constraints) : null,
          onVerticalDragUpdate: !isH ? (d) => _onDrag(d, constraints) : null,
          child: MouseRegion(
            cursor: isH ? SystemMouseCursors.resizeColumn : SystemMouseCursors.resizeRow,
            child: Container(
              width: isH ? _dividerThickness : constraints.maxWidth,
              height: isH ? constraints.maxHeight : _dividerThickness,
              color: const Color(0xFF1f2937),
            ),
          ),
        );

        final secondary = SizedBox(
          width: isH ? secondarySize : constraints.maxWidth,
          height: isH ? constraints.maxHeight : secondarySize,
          child: EditorWebView(onReady: widget.onSecondaryReady, onEvent: widget.onSecondaryEvent),
        );

        return isH
            ? Row(children: [primary, divider, secondary])
            : Column(children: [primary, divider, secondary]);
      },
    );
  }
}
