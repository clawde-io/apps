// SPDX-License-Identifier: MIT
// CodeMirror 6 editor WebView widget (Sprint HH, ED.1).

import 'package:flutter/material.dart';
import 'package:webview_flutter/webview_flutter.dart';

import 'package:clawde/features/editor/js_bridge.dart';

/// A Flutter widget that embeds a CodeMirror 6 editor in a WebView.
///
/// The editor loads `assets/editor/editor.html` which bundles CodeMirror 6
/// with language packs.  Bidirectional communication is handled by [JsBridge].
///
/// Usage:
/// ```dart
/// EditorWebView(
///   onReady: (bridge) => bridge.sendOpenFile(path: 'main.rs', content: '...', language: 'rust'),
///   onEvent: (event) { ... },
/// )
/// ```
class EditorWebView extends StatefulWidget {
  const EditorWebView({
    super.key,
    this.onReady,
    this.onEvent,
    this.initialPath,
    this.initialContent,
    this.initialLanguage,
  });

  /// Called once the WebView is ready and the bridge is initialised.
  final ValueChanged<JsBridge>? onReady;

  /// Called for every event emitted from the editor.
  final ValueChanged<EditorEvent>? onEvent;

  /// Optional file to open immediately after the editor is ready.
  final String? initialPath;
  final String? initialContent;
  final String? initialLanguage;

  @override
  State<EditorWebView> createState() => _EditorWebViewState();
}

class _EditorWebViewState extends State<EditorWebView> {
  late final WebViewController _controller;
  late final JsBridge _bridge;
  bool _ready = false;

  @override
  void initState() {
    super.initState();
    _controller = WebViewController()
      ..setJavaScriptMode(JavaScriptMode.unrestricted)
      ..setBackgroundColor(const Color(0xFF0f0f14))
      ..setNavigationDelegate(
        NavigationDelegate(
          onPageFinished: (_) => _onPageFinished(),
        ),
      )
      ..loadFlutterAsset('assets/editor/editor.html');

    _bridge = JsBridge(
      controller: _controller,
      onEvent: _handleEvent,
    );
    _bridge.setup();
  }

  void _onPageFinished() {
    if (_ready) return;
    setState(() => _ready = true);
    widget.onReady?.call(_bridge);

    // Open initial file if provided.
    if (widget.initialPath != null && widget.initialContent != null) {
      _bridge.sendOpenFile(
        path: widget.initialPath!,
        content: widget.initialContent!,
        language: widget.initialLanguage ?? 'plaintext',
      );
    }
  }

  void _handleEvent(EditorEvent event) {
    widget.onEvent?.call(event);
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        WebViewWidget(controller: _controller),
        if (!_ready)
          const Center(
            child: CircularProgressIndicator(),
          ),
      ],
    );
  }
}
