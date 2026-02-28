// SPDX-License-Identifier: MIT
// Editor commands — go-to-definition, symbol search (Sprint HH, ED.7).

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';

/// Handles Cmd+Click go-to-definition and other editor commands.
///
/// Calls `lsp.definition` RPC; falls back to a daemon search if LSP
/// returns no results.
class EditorCommands {
  const EditorCommands({required this.ref, required this.onOpenFile});

  final WidgetRef ref;

  /// Called when a file should be opened at a specific line.
  final void Function(String path, int line) onOpenFile;

  /// Go to definition for the symbol at [line]:[col] in [filePath].
  ///
  /// First attempts `lsp.definition`; if that returns nothing, falls back
  /// to a simple ripgrep-style search via `repo.searchSymbol`.
  Future<void> goToDefinition({
    required String filePath,
    required int line,
    required int col,
    required String symbol,
  }) async {
    final client = ref.read(daemonProvider.notifier).client;

    // Try LSP first.
    try {
      final result = await client.call<Map<String, dynamic>>(
        'lsp.definition',
        {'filePath': filePath, 'line': line, 'col': col},
      );
      final defFile = result['filePath'] as String?;
      final defLine = result['line'] as int? ?? 0;
      if (defFile != null && defFile.isNotEmpty) {
        onOpenFile(defFile, defLine);
        return;
      }
    } catch (_) {
      // LSP not available — fall through to symbol search.
    }

    // Fallback: repo symbol search.
    try {
      final result = await client.call<Map<String, dynamic>>(
        'repo.searchSymbol',
        {'symbol': symbol},
      );
      final hits = result['hits'] as List<dynamic>? ?? [];
      if (hits.isNotEmpty) {
        final first = hits.first as Map<String, dynamic>;
        final hitFile = first['filePath'] as String? ?? '';
        final hitLine = first['line'] as int? ?? 0;
        if (hitFile.isNotEmpty) {
          onOpenFile(hitFile, hitLine);
        }
      }
    } catch (_) {
      // No results — do nothing.
    }
  }
}

/// An icon button that triggers the global search overlay (Cmd+K).
class GlobalSearchButton extends ConsumerWidget {
  const GlobalSearchButton({super.key, this.onTap});

  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Tooltip(
      message: 'Search sessions (⌘K)',
      child: IconButton(
        icon: const Icon(Icons.search, size: 18),
        onPressed: onTap,
        color: const Color(0xFF9ca3af),
      ),
    );
  }
}
