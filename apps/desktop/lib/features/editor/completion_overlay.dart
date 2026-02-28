import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';

/// Sprint GG CC.6 — Ghost-text completion overlay for the desktop editor.
///
/// Wraps an editor child widget and shows an inline AI completion suggestion
/// in a lighter, italic color.  Tab accepts; Escape dismisses.
class CompletionOverlay extends ConsumerStatefulWidget {
  const CompletionOverlay({
    super.key,
    required this.child,
    required this.sessionId,
    this.onAccept,
    this.onDismiss,
  });

  /// The editor widget this overlay decorates.
  final Widget child;

  /// Active session ID for routing completion requests.
  final String sessionId;

  /// Called with the accepted insertion text.
  final ValueChanged<Insertion>? onAccept;

  /// Called when the suggestion is dismissed.
  final VoidCallback? onDismiss;

  @override
  ConsumerState<CompletionOverlay> createState() => _CompletionOverlayState();
}

class _CompletionOverlayState extends ConsumerState<CompletionOverlay> {
  Insertion? _suggestion;
  bool _loading = false;
  Timer? _debounce;

  // Current editor content for context injection.
  String _prefix = '';
  String _suffix = '';
  String _filePath = '';
  int _cursorLine = 0;
  int _cursorCol = 0;

  static const _debounceMs = Duration(milliseconds: 150);

  @override
  void dispose() {
    _debounce?.cancel();
    super.dispose();
  }

  /// Called by the editor when cursor position or content changes.
  void onEditorChange({
    required String prefix,
    required String suffix,
    required String filePath,
    required int cursorLine,
    required int cursorCol,
    String fileContent = '',
  }) {
    _prefix = prefix;
    _suffix = suffix;
    _filePath = filePath;
    _cursorLine = cursorLine;
    _cursorCol = cursorCol;

    // Dismiss stale suggestion immediately.
    if (_suggestion != null) {
      setState(() => _suggestion = null);
      widget.onDismiss?.call();
    }

    // Debounce completion request.
    _debounce?.cancel();
    _debounce = Timer(_debounceMs, () => _requestCompletion(fileContent));
  }

  Future<void> _requestCompletion(String fileContent) async {
    if (!mounted || widget.sessionId.isEmpty) return;
    if (_prefix.trim().isEmpty) return;

    setState(() => _loading = true);

    try {
      final client = ref.read(daemonProvider.notifier).client;
      final result = await client.call<Map<String, dynamic>>(
        'completion.complete',
        CompletionRequest(
          filePath: _filePath,
          prefix: _prefix,
          suffix: _suffix,
          cursorLine: _cursorLine,
          cursorCol: _cursorCol,
          fileContent: fileContent,
          sessionId: widget.sessionId,
        ).toJson(),
      );

      if (!mounted) return;
      final resp = CompletionResponse.fromJson(result);
      if (resp.hasResults) {
        setState(() => _suggestion = resp.insertions.first);
      }
    } catch (_) {
      // Completion errors are non-fatal — editor still works.
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  void _accept() {
    if (_suggestion == null) return;
    widget.onAccept?.call(_suggestion!);
    setState(() => _suggestion = null);
  }

  void _dismiss() {
    setState(() => _suggestion = null);
    widget.onDismiss?.call();
  }

  @override
  Widget build(BuildContext context) {
    return KeyboardListener(
      focusNode: FocusNode(skipTraversal: true),
      onKeyEvent: (event) {
        if (event is KeyDownEvent) {
          if (event.logicalKey == LogicalKeyboardKey.tab && _suggestion != null) {
            _accept();
          } else if (event.logicalKey == LogicalKeyboardKey.escape && _suggestion != null) {
            _dismiss();
          }
        }
      },
      child: Stack(
        children: [
          widget.child,
          if (_loading)
            const Positioned(
              right: 8,
              top: 8,
              child: SizedBox(
                width: 12,
                height: 12,
                child: CircularProgressIndicator(strokeWidth: 1.5),
              ),
            ),
          if (_suggestion != null) _GhostTextHint(suggestion: _suggestion!),
        ],
      ),
    );
  }
}

/// Renders the ghost-text accept/dismiss hint bar below the editor.
class _GhostTextHint extends StatelessWidget {
  const _GhostTextHint({required this.suggestion});

  final Insertion suggestion;

  @override
  Widget build(BuildContext context) {
    final preview = suggestion.text.split('\n').first;
    return Positioned(
      left: 0,
      right: 0,
      bottom: 0,
      child: Container(
        color: const Color(0xFF1a1a1f),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        child: Row(
          children: [
            Expanded(
              child: Text(
                preview,
                style: const TextStyle(
                  fontFamily: 'monospace',
                  fontSize: 12,
                  color: Color(0xFF6b7280),
                  fontStyle: FontStyle.italic,
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
            const SizedBox(width: 8),
            const _KeyHint(label: 'Tab', sublabel: 'Accept'),
            const SizedBox(width: 8),
            const _KeyHint(label: 'Esc', sublabel: 'Dismiss'),
          ],
        ),
      ),
    );
  }
}

class _KeyHint extends StatelessWidget {
  const _KeyHint({required this.label, required this.sublabel});

  final String label;
  final String sublabel;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
          decoration: BoxDecoration(
            border: Border.all(color: const Color(0xFF374151)),
            borderRadius: BorderRadius.circular(3),
          ),
          child: Text(
            label,
            style: const TextStyle(fontSize: 10, color: Color(0xFF9ca3af)),
          ),
        ),
        const SizedBox(width: 4),
        Text(
          sublabel,
          style: const TextStyle(fontSize: 10, color: Color(0xFF6b7280)),
        ),
      ],
    );
  }
}
