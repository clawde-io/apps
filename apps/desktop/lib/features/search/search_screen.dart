import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/router.dart';

/// Result types returned by the search.
enum _ResultKind { session, message }

class _SearchResult {
  const _SearchResult({
    required this.kind,
    required this.sessionId,
    required this.title,
    required this.snippet,
    this.repoPath,
    this.timestamp,
    this.provider,
  });

  final _ResultKind kind;
  final String sessionId;
  final String title;
  final String snippet;
  final String? repoPath;
  final DateTime? timestamp;
  final ProviderType? provider;
}

class SearchScreen extends ConsumerStatefulWidget {
  const SearchScreen({super.key});

  @override
  ConsumerState<SearchScreen> createState() => _SearchScreenState();
}

class _SearchScreenState extends ConsumerState<SearchScreen> {
  final _controller = TextEditingController();
  final _focusNode = FocusNode();
  Timer? _debounce;
  List<_SearchResult> _results = [];
  bool _isSearching = false;

  @override
  void initState() {
    super.initState();
    // Auto-focus the search field on mount.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _focusNode.requestFocus();
    });
  }

  @override
  void dispose() {
    _controller.dispose();
    _focusNode.dispose();
    _debounce?.cancel();
    super.dispose();
  }

  void _onQueryChanged(String query) {
    _debounce?.cancel();
    if (query.trim().isEmpty) {
      setState(() {
        _results = [];
        _isSearching = false;
      });
      return;
    }
    setState(() => _isSearching = true);
    _debounce = Timer(const Duration(milliseconds: 300), () {
      _performSearch(query.trim());
    });
  }

  Future<void> _performSearch(String query) async {
    final queryLower = query.toLowerCase();
    final results = <_SearchResult>[];

    // Search sessions by title and repo path.
    final sessions = ref.read(sessionListProvider).valueOrNull ?? [];
    for (final session in sessions) {
      final titleMatch = _fuzzyMatch(session.title, queryLower);
      final repoMatch = _fuzzyMatch(session.repoPath, queryLower);
      if (titleMatch || repoMatch) {
        results.add(_SearchResult(
          kind: _ResultKind.session,
          sessionId: session.id,
          title: session.title.isNotEmpty ? session.title : _repoName(session.repoPath),
          snippet: session.repoPath,
          repoPath: session.repoPath,
          timestamp: session.updatedAt,
          provider: session.provider,
        ));
      }
    }

    // Search messages across all known sessions.
    for (final session in sessions) {
      final messages =
          ref.read(messageListProvider(session.id)).valueOrNull ?? [];
      for (final msg in messages) {
        if (_fuzzyMatch(msg.content, queryLower)) {
          final snippetText = _extractSnippet(msg.content, queryLower);
          results.add(_SearchResult(
            kind: _ResultKind.message,
            sessionId: session.id,
            title: session.title.isNotEmpty
                ? session.title
                : _repoName(session.repoPath),
            snippet: snippetText,
            repoPath: session.repoPath,
            timestamp: msg.createdAt,
            provider: session.provider,
          ));
        }
      }
    }

    // Sort by timestamp, most recent first.
    results.sort((a, b) {
      final aTime = a.timestamp ?? DateTime(2000);
      final bTime = b.timestamp ?? DateTime(2000);
      return bTime.compareTo(aTime);
    });

    if (mounted) {
      setState(() {
        _results = results;
        _isSearching = false;
      });
    }
  }

  /// Simple fuzzy match: checks if all characters of the query appear in order
  /// within the target string. Falls back to substring match for short queries.
  bool _fuzzyMatch(String target, String queryLower) {
    final targetLower = target.toLowerCase();
    // Substring match first (most intuitive).
    if (targetLower.contains(queryLower)) return true;
    // Character-order fuzzy match for longer queries.
    if (queryLower.length < 3) return false;
    int qi = 0;
    for (int ti = 0; ti < targetLower.length && qi < queryLower.length; ti++) {
      if (targetLower[ti] == queryLower[qi]) qi++;
    }
    return qi == queryLower.length;
  }

  /// Extract a snippet around the first occurrence of the query in the content.
  String _extractSnippet(String content, String queryLower) {
    final lower = content.toLowerCase();
    final idx = lower.indexOf(queryLower);
    if (idx == -1) {
      return content.length > 120 ? '${content.substring(0, 120)}...' : content;
    }
    final start = (idx - 40).clamp(0, content.length);
    final end = (idx + queryLower.length + 80).clamp(0, content.length);
    final prefix = start > 0 ? '...' : '';
    final suffix = end < content.length ? '...' : '';
    return '$prefix${content.substring(start, end).replaceAll('\n', ' ')}$suffix';
  }

  String _repoName(String path) {
    final parts = path.replaceAll(r'\', '/').split('/');
    return parts.where((p) => p.isNotEmpty).lastOrNull ?? path;
  }

  String _relativeTime(DateTime? dt) {
    if (dt == null) return '';
    final diff = DateTime.now().difference(dt);
    if (diff.inSeconds < 60) return 'just now';
    if (diff.inMinutes < 60) return '${diff.inMinutes}m ago';
    if (diff.inHours < 24) return '${diff.inHours}h ago';
    return '${diff.inDays}d ago';
  }

  void _openResult(_SearchResult result) {
    ref.read(activeSessionIdProvider.notifier).state = result.sessionId;
    context.go(routeChat);
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        // ── Header ───────────────────────────────────────────────────────────
        Container(
          height: 56,
          padding: const EdgeInsets.symmetric(horizontal: 20),
          decoration: const BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            border: Border(
              bottom: BorderSide(color: ClawdTheme.surfaceBorder),
            ),
          ),
          child: Row(
            children: [
              const Text(
                'Search',
                style: TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  color: Colors.white,
                ),
              ),
              const SizedBox(width: 12),
              if (_results.isNotEmpty)
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                  decoration: BoxDecoration(
                    color: ClawdTheme.claw.withValues(alpha: 0.2),
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Text(
                    '${_results.length}',
                    style: const TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w600,
                      color: ClawdTheme.clawLight,
                    ),
                  ),
                ),
              const Spacer(),
              Text(
                'Tip: Use \u2318K to open search from anywhere',
                style: TextStyle(
                  fontSize: 11,
                  color: Colors.white.withValues(alpha: 0.3),
                ),
              ),
            ],
          ),
        ),

        // ── Search input ─────────────────────────────────────────────────────
        Container(
          padding: const EdgeInsets.fromLTRB(20, 12, 20, 12),
          decoration: const BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            border: Border(
              bottom: BorderSide(color: ClawdTheme.surfaceBorder),
            ),
          ),
          child: TextField(
            controller: _controller,
            focusNode: _focusNode,
            onChanged: _onQueryChanged,
            style: const TextStyle(fontSize: 14, color: Colors.white),
            decoration: InputDecoration(
              hintText: 'Search sessions, messages, file paths...',
              hintStyle: TextStyle(
                fontSize: 14,
                color: Colors.white.withValues(alpha: 0.3),
              ),
              prefixIcon: const Icon(Icons.search, size: 20, color: Colors.white38),
              suffixIcon: _controller.text.isNotEmpty
                  ? IconButton(
                      icon: const Icon(Icons.clear, size: 16, color: Colors.white38),
                      onPressed: () {
                        _controller.clear();
                        _onQueryChanged('');
                        _focusNode.requestFocus();
                      },
                    )
                  : null,
              filled: true,
              fillColor: ClawdTheme.surface,
              contentPadding:
                  const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
              border: OutlineInputBorder(
                borderRadius: BorderRadius.circular(8),
                borderSide: const BorderSide(color: ClawdTheme.surfaceBorder),
              ),
              enabledBorder: OutlineInputBorder(
                borderRadius: BorderRadius.circular(8),
                borderSide: const BorderSide(color: ClawdTheme.surfaceBorder),
              ),
              focusedBorder: OutlineInputBorder(
                borderRadius: BorderRadius.circular(8),
                borderSide: const BorderSide(color: ClawdTheme.claw),
              ),
            ),
          ),
        ),

        // ── Results ──────────────────────────────────────────────────────────
        Expanded(
          child: _buildBody(),
        ),
      ],
    );
  }

  Widget _buildBody() {
    if (_controller.text.trim().isEmpty) {
      return const EmptyState(
        icon: Icons.search,
        title: 'Search across your sessions',
        subtitle:
            'Find sessions by name, messages by content, or files by path',
      );
    }

    if (_isSearching) {
      return const Center(child: CircularProgressIndicator(strokeWidth: 2));
    }

    if (_results.isEmpty) {
      return const EmptyState(
        icon: Icons.search_off,
        title: 'No results found',
        subtitle: 'Try a different search term',
      );
    }

    return ListView.builder(
      padding: const EdgeInsets.symmetric(vertical: 8),
      itemCount: _results.length,
      itemBuilder: (context, i) => _ResultTile(
        result: _results[i],
        query: _controller.text.trim(),
        onTap: () => _openResult(_results[i]),
        relativeTime: _relativeTime,
      ),
    );
  }
}

// ── Result tile ───────────────────────────────────────────────────────────────

class _ResultTile extends StatelessWidget {
  const _ResultTile({
    required this.result,
    required this.query,
    required this.onTap,
    required this.relativeTime,
  });

  final _SearchResult result;
  final String query;
  final VoidCallback onTap;
  final String Function(DateTime?) relativeTime;

  @override
  Widget build(BuildContext context) {
    final isSession = result.kind == _ResultKind.session;

    return InkWell(
      onTap: onTap,
      child: Container(
        margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 3),
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        decoration: BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: ClawdTheme.surfaceBorder),
        ),
        child: Row(
          children: [
            // Type icon
            Container(
              width: 32,
              height: 32,
              decoration: BoxDecoration(
                color: (isSession ? ClawdTheme.claw : ClawdTheme.info)
                    .withValues(alpha: 0.15),
                borderRadius: BorderRadius.circular(6),
              ),
              child: Icon(
                isSession ? Icons.layers : Icons.chat_bubble_outline,
                size: 16,
                color: isSession ? ClawdTheme.clawLight : ClawdTheme.info,
              ),
            ),
            const SizedBox(width: 12),

            // Title + snippet
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Expanded(
                        child: Text(
                          result.title,
                          style: const TextStyle(
                            fontSize: 13,
                            fontWeight: FontWeight.w600,
                            color: Colors.white,
                          ),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      if (result.provider != null) ...[
                        const SizedBox(width: 8),
                        ProviderBadge(provider: result.provider!),
                      ],
                    ],
                  ),
                  const SizedBox(height: 3),
                  Text(
                    result.snippet,
                    style: const TextStyle(fontSize: 12, color: Colors.white38),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                ],
              ),
            ),
            const SizedBox(width: 12),

            // Timestamp
            Column(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                  decoration: BoxDecoration(
                    color: (isSession ? ClawdTheme.claw : ClawdTheme.info)
                        .withValues(alpha: 0.1),
                    borderRadius: BorderRadius.circular(4),
                  ),
                  child: Text(
                    isSession ? 'Session' : 'Message',
                    style: TextStyle(
                      fontSize: 10,
                      fontWeight: FontWeight.w600,
                      color: isSession ? ClawdTheme.clawLight : ClawdTheme.info,
                    ),
                  ),
                ),
                const SizedBox(height: 4),
                Text(
                  relativeTime(result.timestamp),
                  style: const TextStyle(fontSize: 11, color: Colors.white38),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
