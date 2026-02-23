import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:clawd_ui/clawd_ui.dart';

// ─── Data ─────────────────────────────────────────────────────────────────────

class _Hunk {
  _Hunk({
    required this.index,
    required this.header,
    required this.lines,
  });

  final int index;

  /// The raw `@@ -a,b +c,d @@` header line.
  final String header;

  /// All lines belonging to this hunk (including the header line).
  final List<String> lines;

  /// User decision: `true` = accepted, `false` = rejected, `null` = undecided.
  bool? accepted;
}

// ─── Dialog ───────────────────────────────────────────────────────────────────

/// Full-screen diff review dialog with per-hunk Accept / Reject controls.
///
/// Opens when the user taps the "Open Full Diff" icon on a [FileEditCard].
/// Each `@@` hunk can be accepted or rejected independently.
/// Keyboard shortcuts: **F7** = next hunk, **Shift+F7** = previous hunk,
/// **Escape** = close.
///
/// Returns a `Map<int, bool>` keyed by hunk index (0-based) containing only
/// the hunks where the user made a decision; undecided hunks are omitted.
///
/// Usage:
/// ```dart
/// final decisions = await DiffReviewDialog.show(
///   context,
///   filePath: 'lib/src/auth.dart',
///   diffContent: rawDiff,
/// );
/// ```
class DiffReviewDialog extends StatefulWidget {
  const DiffReviewDialog({
    super.key,
    required this.filePath,
    required this.diffContent,
    this.onResult,
  });

  final String filePath;
  final String diffContent;

  /// Optional eagerly-called callback when the dialog is dismissed.
  /// Receives the same map that [show] returns.
  final void Function(Map<int, bool> decisions)? onResult;

  /// Parses [diffContent] into hunks and presents the review dialog.
  static Future<Map<int, bool>?> show(
    BuildContext context, {
    required String filePath,
    required String diffContent,
  }) {
    return showDialog<Map<int, bool>>(
      context: context,
      builder: (_) => DiffReviewDialog(
        filePath: filePath,
        diffContent: diffContent,
      ),
    );
  }

  @override
  State<DiffReviewDialog> createState() => _DiffReviewDialogState();
}

class _DiffReviewDialogState extends State<DiffReviewDialog> {
  late final List<_Hunk> _hunks;
  int _focusedIndex = 0;
  final _scrollController = ScrollController();
  final _itemKeys = <int, GlobalKey>{};

  @override
  void initState() {
    super.initState();
    _hunks = _parseHunks(widget.diffContent);
    for (var i = 0; i < _hunks.length; i++) {
      _itemKeys[i] = GlobalKey();
    }
  }

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  void _nextHunk() {
    if (_focusedIndex < _hunks.length - 1) {
      setState(() => _focusedIndex++);
      _scrollToFocused();
    }
  }

  void _prevHunk() {
    if (_focusedIndex > 0) {
      setState(() => _focusedIndex--);
      _scrollToFocused();
    }
  }

  void _scrollToFocused() {
    final key = _itemKeys[_focusedIndex];
    if (key?.currentContext == null) return;
    Scrollable.ensureVisible(
      key!.currentContext!,
      duration: const Duration(milliseconds: 200),
      curve: Curves.easeOut,
    );
  }

  void _setDecision(int index, bool? accepted) =>
      setState(() => _hunks[index].accepted = accepted);

  Map<int, bool> _buildDecisions() => {
        for (final h in _hunks)
          if (h.accepted != null) h.index: h.accepted!,
      };

  void _done() {
    final decisions = _buildDecisions();
    widget.onResult?.call(decisions);
    Navigator.of(context).pop(decisions);
  }

  @override
  Widget build(BuildContext context) {
    return Dialog.fullscreen(
      child: Focus(
        autofocus: true,
        onKeyEvent: (_, event) {
          if (event is! KeyDownEvent) return KeyEventResult.ignored;
          if (event.logicalKey == LogicalKeyboardKey.f7) {
            if (HardwareKeyboard.instance.isShiftPressed) {
              _prevHunk();
            } else {
              _nextHunk();
            }
            return KeyEventResult.handled;
          }
          if (event.logicalKey == LogicalKeyboardKey.escape) {
            _done();
            return KeyEventResult.handled;
          }
          return KeyEventResult.ignored;
        },
        child: Scaffold(
          backgroundColor: ClawdTheme.surface,
          appBar: _buildAppBar(),
          body: _hunks.isEmpty ? _buildEmptyState() : _buildHunkList(),
        ),
      ),
    );
  }

  PreferredSizeWidget _buildAppBar() {
    final total = _hunks.length;
    final accepted = _hunks.where((h) => h.accepted == true).length;
    final rejected = _hunks.where((h) => h.accepted == false).length;

    final summaryParts = [
      '$total ${total == 1 ? 'hunk' : 'hunks'}',
      if (accepted > 0) '$accepted accepted',
      if (rejected > 0) '$rejected rejected',
    ];

    return AppBar(
      backgroundColor: ClawdTheme.surfaceElevated,
      elevation: 0,
      leading: IconButton(
        icon: const Icon(Icons.close),
        tooltip: 'Close (Esc)',
        onPressed: _done,
      ),
      title: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            widget.filePath,
            style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600),
            overflow: TextOverflow.ellipsis,
          ),
          Text(
            summaryParts.join('  •  '),
            style: const TextStyle(fontSize: 11, color: Colors.white54),
          ),
        ],
      ),
      actions: [
        const Center(
          child: Padding(
            padding: EdgeInsets.only(right: 8),
            child: Text(
              'F7 next  •  ⇧F7 prev',
              style: TextStyle(fontSize: 10, color: Colors.white38),
            ),
          ),
        ),
        Padding(
          padding: const EdgeInsets.only(right: 12),
          child: FilledButton(
            onPressed: _done,
            style: FilledButton.styleFrom(
              backgroundColor: ClawdTheme.claw,
              foregroundColor: Colors.white,
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
            ),
            child: const Text('Done'),
          ),
        ),
      ],
    );
  }

  Widget _buildEmptyState() {
    return const Center(
      child: Text(
        'No hunks found in this diff.',
        style: TextStyle(color: Colors.white38),
      ),
    );
  }

  Widget _buildHunkList() {
    return ListView.builder(
      controller: _scrollController,
      padding: const EdgeInsets.all(16),
      itemCount: _hunks.length,
      itemBuilder: (context, index) => _HunkCard(
        key: _itemKeys[index],
        hunk: _hunks[index],
        isFocused: index == _focusedIndex,
        onTap: () => setState(() => _focusedIndex = index),
        onAccept: () => _setDecision(index, true),
        onReject: () => _setDecision(index, false),
        onClear: () => _setDecision(index, null),
      ),
    );
  }
}

// ─── Hunk card ────────────────────────────────────────────────────────────────

class _HunkCard extends StatelessWidget {
  const _HunkCard({
    super.key,
    required this.hunk,
    required this.isFocused,
    required this.onTap,
    required this.onAccept,
    required this.onReject,
    required this.onClear,
  });

  final _Hunk hunk;
  final bool isFocused;
  final VoidCallback onTap;
  final VoidCallback onAccept;
  final VoidCallback onReject;
  final VoidCallback onClear;

  Color get _borderColor {
    if (hunk.accepted == true) return ClawdTheme.success;
    if (hunk.accepted == false) return ClawdTheme.error;
    if (isFocused) return ClawdTheme.claw;
    return ClawdTheme.surfaceBorder;
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        margin: const EdgeInsets.only(bottom: 12),
        decoration: BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(
            color: _borderColor,
            width: isFocused ? 1.5 : 1,
          ),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _buildHeader(),
            _buildDiffLines(),
            _buildActions(),
          ],
        ),
      ),
    );
  }

  Widget _buildHeader() {
    final linesAdded = hunk.lines
        .where((l) => l.startsWith('+') && !l.startsWith('+++'))
        .length;
    final linesRemoved = hunk.lines
        .where((l) => l.startsWith('-') && !l.startsWith('---'))
        .length;

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      decoration: BoxDecoration(
        color: ClawdTheme.info.withValues(alpha: 0.08),
        borderRadius: const BorderRadius.vertical(top: Radius.circular(8)),
        border: const Border(
          bottom: BorderSide(color: ClawdTheme.surfaceBorder),
        ),
      ),
      child: Row(
        children: [
          Text(
            'Hunk ${hunk.index + 1}',
            style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w600,
              color: ClawdTheme.info,
            ),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              hunk.header,
              style: const TextStyle(
                fontSize: 10,
                fontFamily: 'monospace',
                color: Colors.white38,
              ),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          if (linesAdded > 0) ...[
            Text(
              '+$linesAdded',
              style: const TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w600,
                color: ClawdTheme.success,
              ),
            ),
            const SizedBox(width: 4),
          ],
          if (linesRemoved > 0)
            Text(
              '-$linesRemoved',
              style: const TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w600,
                color: ClawdTheme.error,
              ),
            ),
          if (hunk.accepted != null) ...[
            const SizedBox(width: 8),
            _DecisionBadge(accepted: hunk.accepted!),
          ],
        ],
      ),
    );
  }

  Widget _buildDiffLines() {
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: IntrinsicWidth(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: hunk.lines.map((l) => _HunkDiffLine(line: l)).toList(),
        ),
      ),
    );
  }

  Widget _buildActions() {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      decoration: const BoxDecoration(
        border: Border(top: BorderSide(color: ClawdTheme.surfaceBorder)),
      ),
      child: Row(
        children: [
          if (hunk.accepted != null)
            TextButton(
              onPressed: onClear,
              style: TextButton.styleFrom(
                minimumSize: Size.zero,
                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              ),
              child: const Text(
                'Undo',
                style: TextStyle(fontSize: 12, color: Colors.white38),
              ),
            ),
          const Spacer(),
          TextButton(
            onPressed: hunk.accepted == false ? null : onReject,
            style: TextButton.styleFrom(
              minimumSize: Size.zero,
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
            ),
            child: Text(
              'Reject',
              style: TextStyle(
                fontSize: 12,
                color: hunk.accepted == false ? ClawdTheme.error : Colors.white54,
                fontWeight:
                    hunk.accepted == false ? FontWeight.w600 : FontWeight.normal,
              ),
            ),
          ),
          const SizedBox(width: 8),
          FilledButton(
            onPressed: hunk.accepted == true ? null : onAccept,
            style: FilledButton.styleFrom(
              backgroundColor: hunk.accepted == true
                  ? ClawdTheme.success.withValues(alpha: 0.5)
                  : ClawdTheme.success,
              foregroundColor: Colors.white,
              minimumSize: Size.zero,
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 6),
            ),
            child: const Text('Accept', style: TextStyle(fontSize: 12)),
          ),
        ],
      ),
    );
  }
}

// ─── Decision badge ───────────────────────────────────────────────────────────

class _DecisionBadge extends StatelessWidget {
  const _DecisionBadge({required this.accepted});
  final bool accepted;

  @override
  Widget build(BuildContext context) {
    final color = accepted ? ClawdTheme.success : ClawdTheme.error;
    final label = accepted ? 'Accepted' : 'Rejected';
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(4),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontSize: 10,
          color: color,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}

// ─── Diff line ────────────────────────────────────────────────────────────────

class _HunkDiffLine extends StatelessWidget {
  const _HunkDiffLine({required this.line});
  final String line;

  Color get _bgColor {
    if (line.startsWith('+') && !line.startsWith('+++')) {
      return ClawdTheme.success.withValues(alpha: 0.08);
    }
    if (line.startsWith('-') && !line.startsWith('---')) {
      return ClawdTheme.error.withValues(alpha: 0.08);
    }
    if (line.startsWith('@@')) {
      return ClawdTheme.info.withValues(alpha: 0.06);
    }
    return Colors.transparent;
  }

  Color get _textColor {
    if (line.startsWith('+') && !line.startsWith('+++')) return ClawdTheme.success;
    if (line.startsWith('-') && !line.startsWith('---')) return ClawdTheme.error;
    if (line.startsWith('@@')) return ClawdTheme.info;
    return Colors.white54;
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      color: _bgColor,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 1),
      child: Text(
        line.isEmpty ? ' ' : line,
        style: TextStyle(
          fontSize: 11,
          fontFamily: 'monospace',
          color: _textColor,
          height: 1.6,
        ),
      ),
    );
  }
}

// ─── Hunk parser ──────────────────────────────────────────────────────────────

/// Splits a unified diff string into a list of [_Hunk] objects.
///
/// Lines before the first `@@` marker (e.g. `---`/`+++` file headers) are
/// discarded because they are redundant with the dialog's own file-path header.
List<_Hunk> _parseHunks(String diff) {
  final lines = diff.split('\n');
  final hunks = <_Hunk>[];
  List<String>? currentLines;
  String currentHeader = '';

  for (final line in lines) {
    if (line.startsWith('@@')) {
      if (currentLines != null && currentLines.isNotEmpty) {
        hunks.add(_Hunk(
          index: hunks.length,
          header: currentHeader,
          lines: List.unmodifiable(currentLines),
        ));
      }
      currentHeader = line;
      currentLines = [line];
    } else if (currentLines != null) {
      currentLines.add(line);
    }
  }

  if (currentLines != null && currentLines.isNotEmpty) {
    hunks.add(_Hunk(
      index: hunks.length,
      header: currentHeader,
      lines: List.unmodifiable(currentLines),
    ));
  }

  return hunks;
}
