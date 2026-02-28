// SPDX-License-Identifier: MIT
// Two-pane diff viewer using CodeMirror @codemirror/merge (Sprint HH, ED.6).

import 'package:flutter/material.dart';
import 'package:webview_flutter/webview_flutter.dart';

/// A side-by-side diff viewer powered by CodeMirror 6's `@codemirror/merge`.
///
/// Loads `assets/editor/diff.html` which bundles the merge extension.
/// Pass [original] and [modified] to display the diff.
class DiffEditorWebView extends StatefulWidget {
  const DiffEditorWebView({
    super.key,
    required this.original,
    required this.modified,
    this.language = 'plaintext',
    this.filePath,
  });

  /// Original (left pane) content.
  final String original;

  /// Modified (right pane) content.
  final String modified;

  /// Language for syntax highlighting (e.g. `"rust"`, `"typescript"`).
  final String language;

  /// Optional file path for the title bar.
  final String? filePath;

  @override
  State<DiffEditorWebView> createState() => _DiffEditorWebViewState();
}

class _DiffEditorWebViewState extends State<DiffEditorWebView> {
  late final WebViewController _controller;
  bool _ready = false;

  @override
  void initState() {
    super.initState();
    _controller = WebViewController()
      ..setJavaScriptMode(JavaScriptMode.unrestricted)
      ..setBackgroundColor(const Color(0xFF0f0f14))
      ..setNavigationDelegate(
        NavigationDelegate(onPageFinished: (_) => _injectDiff()),
      )
      ..loadFlutterAsset('assets/editor/diff.html');
  }

  @override
  void didUpdateWidget(DiffEditorWebView old) {
    super.didUpdateWidget(old);
    if (old.original != widget.original || old.modified != widget.modified) {
      _injectDiff();
    }
  }

  Future<void> _injectDiff() async {
    if (!_ready) {
      setState(() => _ready = true);
    }
    // Escape the strings for safe injection.
    final orig = _jsEscape(widget.original);
    final mod = _jsEscape(widget.modified);
    await _controller.runJavaScript(
      'window.clawdDiff && window.clawdDiff("$orig", "$mod", "${widget.language}")',
    );
  }

  String _jsEscape(String s) => s
      .replaceAll('\\', '\\\\')
      .replaceAll('"', '\\"')
      .replaceAll('\n', '\\n')
      .replaceAll('\r', '\\r');

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        if (widget.filePath != null)
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
            color: const Color(0xFF0d0d12),
            child: Row(
              children: [
                const Icon(Icons.compare, size: 14, color: Color(0xFF6b7280)),
                const SizedBox(width: 8),
                Text(
                  widget.filePath!,
                  style: const TextStyle(fontSize: 12, color: Color(0xFF9ca3af)),
                ),
              ],
            ),
          ),
        Expanded(child: WebViewWidget(controller: _controller)),
      ],
    );
  }
}
