import 'package:flutter/material.dart';
import 'package:flutter_markdown/flutter_markdown.dart';
import 'package:flutter_highlight/flutter_highlight.dart';
import 'package:flutter_highlight/themes/atom-one-dark.dart';
import 'package:markdown/markdown.dart' as md;
import '../theme/clawd_theme.dart';

/// Renders markdown content — used inside [ChatBubble] for assistant messages.
class MarkdownMessage extends StatelessWidget {
  const MarkdownMessage({super.key, required this.content});

  final String content;

  @override
  Widget build(BuildContext context) {
    return MarkdownBody(
      data: content,
      selectable: true,
      extensionSet: md.ExtensionSet.gitHubFlavored,
      styleSheet: MarkdownStyleSheet(
        p: const TextStyle(fontSize: 14, height: 1.5, color: Colors.white),
        strong: const TextStyle(
          fontSize: 14,
          height: 1.5,
          color: Colors.white,
          fontWeight: FontWeight.bold,
        ),
        em: const TextStyle(
          fontSize: 14,
          height: 1.5,
          color: Colors.white,
          fontStyle: FontStyle.italic,
        ),
        code: TextStyle(
          fontFamily: 'monospace',
          fontSize: 13,
          color: ClawdTheme.clawLight,
          backgroundColor: ClawdTheme.surface,
        ),
        codeblockDecoration: BoxDecoration(
          color: ClawdTheme.surface,
          borderRadius: BorderRadius.circular(6),
          border: Border.all(color: ClawdTheme.surfaceBorder),
        ),
        blockquoteDecoration: BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          border: Border(
            left: BorderSide(color: ClawdTheme.clawLight, width: 3),
          ),
        ),
        h1: const TextStyle(
          fontSize: 20,
          fontWeight: FontWeight.bold,
          color: Colors.white,
        ),
        h2: const TextStyle(
          fontSize: 18,
          fontWeight: FontWeight.bold,
          color: Colors.white,
        ),
        h3: const TextStyle(
          fontSize: 16,
          fontWeight: FontWeight.bold,
          color: Colors.white,
        ),
        listBullet: const TextStyle(fontSize: 14, color: Colors.white),
        tableBody: const TextStyle(fontSize: 13, color: Colors.white),
        tableHead: const TextStyle(
          fontSize: 13,
          fontWeight: FontWeight.bold,
          color: Colors.white,
        ),
      ),
      builders: {'code': _SyntaxHighlightBuilder()},
    );
  }
}

class _SyntaxHighlightBuilder extends MarkdownElementBuilder {
  @override
  Widget? visitElementAfterWithContext(
    BuildContext context,
    md.Element element,
    TextStyle? preferredStyle,
    TextStyle? parentStyle,
  ) {
    final String code = element.textContent;
    // Extract language from class attribute (e.g. "language-dart")
    final String? langClass = element.attributes['class'];
    final String language = langClass != null && langClass.startsWith('language-')
        ? langClass.substring('language-'.length)
        : 'plaintext';

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: HighlightView(
        code.trimRight(),
        language: language,
        theme: atomOneDarkTheme,
        padding: const EdgeInsets.all(12),
        textStyle: const TextStyle(fontFamily: 'monospace', fontSize: 13),
      ),
    );
  }
}
