import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';

/// Sprint GG SS.4 — Global session search bar (Cmd+K).
///
/// Modal overlay that lets the user search across all session messages.
/// Results are BM25-ranked and show session name + snippet + timestamp.
/// Clicking a result navigates to the session and scrolls to the message.
///
/// Usage:
/// ```dart
/// void _openSearch(BuildContext context) {
///   showDialog(context: context, builder: (_) => const GlobalSearchDialog());
/// }
/// ```
class GlobalSearchDialog extends ConsumerStatefulWidget {
  const GlobalSearchDialog({super.key});

  @override
  ConsumerState<GlobalSearchDialog> createState() => _GlobalSearchDialogState();
}

class _GlobalSearchDialogState extends ConsumerState<GlobalSearchDialog> {
  final _controller = TextEditingController();
  final _scrollController = ScrollController();
  List<SearchResult> _results = [];
  bool _loading = false;
  Timer? _debounce;
  int _selected = 0;

  static const _debounceMs = Duration(milliseconds: 200);

  @override
  void dispose() {
    _controller.dispose();
    _scrollController.dispose();
    _debounce?.cancel();
    super.dispose();
  }

  void _onQueryChanged(String query) {
    _debounce?.cancel();
    if (query.trim().isEmpty) {
      setState(() {
        _results = [];
        _loading = false;
        _selected = 0;
      });
      return;
    }
    setState(() => _loading = true);
    _debounce = Timer(_debounceMs, () => _search(query));
  }

  Future<void> _search(String query) async {
    if (!mounted) return;
    try {
      final client = ref.read(daemonProvider.notifier).client;
      final result = await client.call<Map<String, dynamic>>(
        'session.search',
        SearchQuery(query: query, limit: 30).toJson(),
      );
      if (!mounted) return;
      final resp = SearchResponse.fromJson(result);
      setState(() {
        _results = resp.results;
        _loading = false;
        _selected = 0;
      });
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  void _selectResult(SearchResult result) {
    Navigator.of(context).pop(result);
  }

  void _handleKeyEvent(KeyEvent event) {
    if (event is! KeyDownEvent) return;
    if (event.logicalKey == LogicalKeyboardKey.arrowDown) {
      setState(() => _selected = (_selected + 1).clamp(0, _results.length - 1));
    } else if (event.logicalKey == LogicalKeyboardKey.arrowUp) {
      setState(() => _selected = (_selected - 1).clamp(0, _results.length - 1));
    } else if (event.logicalKey == LogicalKeyboardKey.enter && _results.isNotEmpty) {
      _selectResult(_results[_selected]);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: Colors.transparent,
      insetPadding: const EdgeInsets.symmetric(horizontal: 80, vertical: 120),
      child: KeyboardListener(
        focusNode: FocusNode()..requestFocus(),
        onKeyEvent: _handleKeyEvent,
        child: Container(
          constraints: const BoxConstraints(maxWidth: 640, maxHeight: 480),
          decoration: BoxDecoration(
            color: const Color(0xFF111118),
            borderRadius: BorderRadius.circular(12),
            border: Border.all(color: Colors.white.withValues(alpha: 0.1)),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.6),
                blurRadius: 40,
                offset: const Offset(0, 16),
              ),
            ],
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              _SearchInput(controller: _controller, onChanged: _onQueryChanged, loading: _loading),
              if (_results.isNotEmpty) const Divider(height: 1, color: Color(0xFF1f2937)),
              if (_results.isNotEmpty)
                Flexible(
                  child: ListView.builder(
                    controller: _scrollController,
                    shrinkWrap: true,
                    itemCount: _results.length,
                    itemBuilder: (context, i) => _ResultTile(
                      result: _results[i],
                      isSelected: i == _selected,
                      onTap: () => _selectResult(_results[i]),
                    ),
                  ),
                ),
              if (_results.isEmpty && _controller.text.isNotEmpty && !_loading)
                const _EmptyState(),
            ],
          ),
        ),
      ),
    );
  }
}

// ─── Sub-widgets ──────────────────────────────────────────────────────────────

class _SearchInput extends StatelessWidget {
  const _SearchInput({
    required this.controller,
    required this.onChanged,
    required this.loading,
  });

  final TextEditingController controller;
  final ValueChanged<String> onChanged;
  final bool loading;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      child: Row(
        children: [
          loading
              ? const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.search, color: Color(0xFF6b7280), size: 20),
          const SizedBox(width: 12),
          Expanded(
            child: TextField(
              controller: controller,
              onChanged: onChanged,
              autofocus: true,
              style: const TextStyle(fontSize: 15, color: Colors.white),
              decoration: const InputDecoration(
                hintText: 'Search sessions…',
                hintStyle: TextStyle(color: Color(0xFF6b7280)),
                border: InputBorder.none,
                isDense: true,
              ),
            ),
          ),
          const _KbdChip(label: 'Esc'),
        ],
      ),
    );
  }
}

class _ResultTile extends StatelessWidget {
  const _ResultTile({
    required this.result,
    required this.isSelected,
    required this.onTap,
  });

  final SearchResult result;
  final bool isSelected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    // Strip HTML bold tags from snippet for plain rendering.
    final snippet = result.snippet.replaceAll('<b>', '').replaceAll('</b>', '').replaceAll('…', '…');

    return InkWell(
      onTap: onTap,
      child: Container(
        color: isSelected ? const Color(0xFF1f2937) : Colors.transparent,
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
        child: Row(
          children: [
            Icon(
              result.role == 'user' ? Icons.person_outline : Icons.smart_toy_outlined,
              size: 16,
              color: const Color(0xFF6b7280),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    snippet,
                    style: const TextStyle(fontSize: 13, color: Colors.white),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 2),
                  Text(
                    _formatDate(result.createdAt),
                    style: const TextStyle(fontSize: 11, color: Color(0xFF6b7280)),
                  ),
                ],
              ),
            ),
            if (isSelected)
              const Icon(Icons.keyboard_return, size: 14, color: Color(0xFF6b7280)),
          ],
        ),
      ),
    );
  }

  String _formatDate(String iso) {
    try {
      final dt = DateTime.parse(iso).toLocal();
      return '${dt.year}-${dt.month.toString().padLeft(2, '0')}-${dt.day.toString().padLeft(2, '0')} '
          '${dt.hour.toString().padLeft(2, '0')}:${dt.minute.toString().padLeft(2, '0')}';
    } catch (_) {
      return iso;
    }
  }
}

class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    return const Padding(
      padding: EdgeInsets.all(32),
      child: Text(
        'No results found.',
        style: TextStyle(color: Color(0xFF6b7280), fontSize: 13),
        textAlign: TextAlign.center,
      ),
    );
  }
}

class _KbdChip extends StatelessWidget {
  const _KbdChip({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
      decoration: BoxDecoration(
        border: Border.all(color: const Color(0xFF374151)),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(label, style: const TextStyle(fontSize: 10, color: Color(0xFF9ca3af))),
    );
  }
}
