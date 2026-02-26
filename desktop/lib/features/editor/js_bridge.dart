// SPDX-License-Identifier: MIT
// JavaScript bridge for the CodeMirror 6 editor WebView (Sprint HH, ED.2+ED.3).

import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:webview_flutter/webview_flutter.dart';

// ─── Event types (JS → Flutter) ──────────────────────────────────────────────

/// An event emitted from the CodeMirror 6 editor via postMessage.
@immutable
class EditorEvent {
  const EditorEvent({required this.type, this.content, this.path, this.cursorLine, this.cursorCol});

  factory EditorEvent.fromJson(Map<String, dynamic> json) => EditorEvent(
        type: json['type'] as String? ?? '',
        content: json['content'] as String?,
        path: json['path'] as String?,
        cursorLine: json['cursorLine'] as int?,
        cursorCol: json['cursorCol'] as int?,
      );

  /// Event type: `"change"` | `"save"` | `"cursorMove"` | `"ready"`
  final String type;

  /// Updated document content (for `"change"` events).
  final String? content;

  /// File path (for `"save"` events).
  final String? path;

  final int? cursorLine;
  final int? cursorCol;
}

// ─── JsBridge ─────────────────────────────────────────────────────────────────

/// Manages the bidirectional JavaScript bridge between Flutter and CodeMirror 6.
///
/// Call [setup] after the WebView is created to register the message channel.
/// Use [sendOpenFile] to load a file into the editor.
/// The [onEvent] callback is invoked for every message from the editor.
class JsBridge {
  JsBridge({required this.controller, this.onEvent});

  final WebViewController controller;
  final ValueChanged<EditorEvent>? onEvent;

  /// Register the JavaScript message channel and expose the `clawd` object.
  void setup() {
    controller.addJavaScriptChannel(
      'ClawdBridge',
      onMessageReceived: (message) {
        try {
          final json = jsonDecode(message.message) as Map<String, dynamic>;
          onEvent?.call(EditorEvent.fromJson(json));
        } catch (_) {
          // Malformed message — ignore.
        }
      },
    );
  }

  /// Tell the editor to open a file with the given content and language.
  ///
  /// The JS side should listen for `window.clawd_open` events.
  Future<void> sendOpenFile({
    required String path,
    required String content,
    required String language,
  }) async {
    final payload = jsonEncode({'type': 'open', 'path': path, 'content': content, 'language': language});
    await controller.runJavaScript('window.clawdOpen($payload)');
  }

  /// Apply a diff/patch to the current editor content.
  Future<void> sendPatch(String unifiedDiff) async {
    final payload = jsonEncode({'type': 'patch', 'diff': unifiedDiff});
    await controller.runJavaScript('window.clawdPatch($payload)');
  }

  /// Set gutter annotations — lines marked as read/written by an AI session.
  Future<void> sendGutterAnnotations(List<GutterAnnotation> annotations) async {
    final payload = jsonEncode({'type': 'gutter', 'annotations': annotations.map((a) => a.toJson()).toList()});
    await controller.runJavaScript('window.clawdGutter($payload)');
  }

  /// Focus the editor.
  Future<void> focus() => controller.runJavaScript('window.clawdFocus && window.clawdFocus()');

  /// Read the current editor content.
  Future<String> getContent() async {
    final result = await controller.runJavaScriptReturningResult('window.clawdGetContent && window.clawdGetContent()');
    return result.toString();
  }
}

/// A single gutter annotation for the session-aware gutter (ED.9).
class GutterAnnotation {
  const GutterAnnotation({required this.line, required this.kind, required this.sessionId});

  /// 0-based line number.
  final int line;

  /// `"read"` | `"write"` | `"both"`
  final String kind;

  final String sessionId;

  Map<String, dynamic> toJson() => {'line': line, 'kind': kind, 'sessionId': sessionId};
}
