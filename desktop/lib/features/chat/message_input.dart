import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/features/repo/repo_context_provider.dart';

/// Desktop message input with @mention file autocomplete.
///
/// When the user types `@` followed by characters, a dropdown appears
/// showing matching file paths from the active repository. Selecting a
/// file inserts its path as a context reference (e.g. `@src/main.dart`).
class MentionMessageInput extends ConsumerStatefulWidget {
  const MentionMessageInput({
    super.key,
    required this.sessionId,
    required this.session,
  });

  final String sessionId;
  final Session session;

  @override
  ConsumerState<MentionMessageInput> createState() =>
      _MentionMessageInputState();
}

class _MentionMessageInputState extends ConsumerState<MentionMessageInput> {
  final _controller = TextEditingController();
  final _focusNode = FocusNode();
  final _layerLink = LayerLink();

  OverlayEntry? _overlayEntry;
  List<FileStatus> _suggestions = [];
  int _mentionStart = -1;
  int _selectedIndex = 0;

  @override
  void initState() {
    super.initState();
    _controller.addListener(_onTextChanged);
  }

  @override
  void dispose() {
    _removeOverlay();
    _controller.removeListener(_onTextChanged);
    _controller.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  void _onTextChanged() {
    final text = _controller.text;
    final cursorPos = _controller.selection.baseOffset;
    if (cursorPos < 0) {
      _removeOverlay();
      return;
    }

    // Find the `@` before the cursor that starts a mention.
    final beforeCursor = text.substring(0, cursorPos);
    final atIndex = beforeCursor.lastIndexOf('@');

    if (atIndex == -1) {
      _removeOverlay();
      return;
    }

    // Ensure the `@` is at the start or preceded by whitespace.
    if (atIndex > 0 && beforeCursor[atIndex - 1] != ' ') {
      _removeOverlay();
      return;
    }

    final query = beforeCursor.substring(atIndex + 1).toLowerCase();
    _mentionStart = atIndex;

    // Look up files from the active repo status.
    final repoStatus = ref.read(activeRepoStatusProvider).valueOrNull;
    if (repoStatus == null) {
      _removeOverlay();
      return;
    }

    final matches = repoStatus.files
        .where((f) => f.path.toLowerCase().contains(query))
        .take(8)
        .toList();

    if (matches.isEmpty) {
      _removeOverlay();
      return;
    }

    setState(() {
      _suggestions = matches;
      _selectedIndex = 0;
    });
    _showOverlay();
  }

  void _showOverlay() {
    // M11: Guard against inserting into the overlay after the widget has been
    // disposed. The TextEditingController listener (_onTextChanged) can fire
    // after dispose() completes (before removeListener takes effect), so an
    // explicit mounted check is needed here.
    if (!mounted) return;
    _removeOverlay();
    _overlayEntry = OverlayEntry(builder: (_) => _buildOverlay());
    Overlay.of(context).insert(_overlayEntry!);
  }

  void _removeOverlay() {
    _overlayEntry?.remove();
    _overlayEntry = null;
    _mentionStart = -1;
    _suggestions = [];
  }

  void _selectSuggestion(FileStatus file) {
    final text = _controller.text;
    final cursorPos = _controller.selection.baseOffset;
    if (_mentionStart < 0 || cursorPos < 0) return;

    // Replace from @ to cursor with the file path.
    final before = text.substring(0, _mentionStart);
    final after = text.substring(cursorPos);
    final inserted = '@${file.path} ';
    _controller.text = '$before$inserted$after';
    _controller.selection = TextSelection.collapsed(
      offset: before.length + inserted.length,
    );

    _removeOverlay();
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent) return KeyEventResult.ignored;

    // When the overlay is visible, arrow keys and Enter navigate suggestions.
    if (_overlayEntry != null && _suggestions.isNotEmpty) {
      if (event.logicalKey == LogicalKeyboardKey.arrowDown) {
        setState(() {
          _selectedIndex = (_selectedIndex + 1) % _suggestions.length;
        });
        _overlayEntry?.markNeedsBuild();
        return KeyEventResult.handled;
      }
      if (event.logicalKey == LogicalKeyboardKey.arrowUp) {
        setState(() {
          _selectedIndex =
              (_selectedIndex - 1 + _suggestions.length) % _suggestions.length;
        });
        _overlayEntry?.markNeedsBuild();
        return KeyEventResult.handled;
      }
      if (event.logicalKey == LogicalKeyboardKey.tab ||
          (event.logicalKey == LogicalKeyboardKey.enter &&
              !HardwareKeyboard.instance.isShiftPressed)) {
        _selectSuggestion(_suggestions[_selectedIndex]);
        return KeyEventResult.handled;
      }
      if (event.logicalKey == LogicalKeyboardKey.escape) {
        _removeOverlay();
        return KeyEventResult.handled;
      }
    }

    // Normal Enter sends message (when overlay is not visible).
    if (event.logicalKey == LogicalKeyboardKey.enter &&
        !HardwareKeyboard.instance.isShiftPressed &&
        _overlayEntry == null) {
      _send();
      return KeyEventResult.handled;
    }

    return KeyEventResult.ignored;
  }

  void _send() {
    final text = _controller.text.trim();
    if (text.isEmpty) return;
    final isLoading = widget.session.status == SessionStatus.running;
    if (isLoading) return;
    _controller.clear();
    _removeOverlay();
    ref.read(messageListProvider(widget.sessionId).notifier).send(text);
  }

  Widget _buildOverlay() {
    return Positioned(
      width: 320,
      child: CompositedTransformFollower(
        link: _layerLink,
        showWhenUnlinked: false,
        offset: const Offset(0, -8),
        followerAnchor: Alignment.bottomLeft,
        targetAnchor: Alignment.topLeft,
        child: Material(
          elevation: 8,
          color: ClawdTheme.surfaceElevated,
          borderRadius: BorderRadius.circular(8),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxHeight: 240),
            child: ListView.builder(
              padding: const EdgeInsets.symmetric(vertical: 4),
              shrinkWrap: true,
              itemCount: _suggestions.length,
              itemBuilder: (context, index) {
                final file = _suggestions[index];
                final isSelected = index == _selectedIndex;
                return InkWell(
                  onTap: () => _selectSuggestion(file),
                  child: Container(
                    color: isSelected
                        ? ClawdTheme.claw.withValues(alpha: 0.2)
                        : Colors.transparent,
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 6,
                    ),
                    child: Row(
                      children: [
                        const Icon(
                          Icons.insert_drive_file_outlined,
                          size: 14,
                          color: Colors.white38,
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            file.path,
                            style: const TextStyle(
                              fontSize: 13,
                              color: Colors.white70,
                            ),
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ],
                    ),
                  ),
                );
              },
            ),
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final isLoading = widget.session.status == SessionStatus.running;
    final isError = widget.session.status == SessionStatus.error;

    if (isError) {
      return Container(
        padding: const EdgeInsets.all(12),
        decoration: const BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          border: Border(top: BorderSide(color: ClawdTheme.surfaceBorder)),
        ),
        child: Row(
          children: [
            const Icon(Icons.warning_amber,
                color: ClawdTheme.warning, size: 16),
            const SizedBox(width: 8),
            const Expanded(
              child: Text(
                'This session encountered an error.',
                style: TextStyle(fontSize: 13, color: ClawdTheme.warning),
              ),
            ),
            TextButton(
              onPressed: () =>
                  ref.read(sessionListProvider.notifier).resume(widget.sessionId),
              child: const Text('Resume'),
            ),
          ],
        ),
      );
    }

    return CompositedTransformTarget(
      link: _layerLink,
      child: Container(
        padding: const EdgeInsets.all(12),
        decoration: const BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          border:
              Border(top: BorderSide(color: ClawdTheme.surfaceBorder)),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            Expanded(
              child: Focus(
                onKeyEvent: _onKey,
                child: TextField(
                  controller: _controller,
                  focusNode: _focusNode,
                  minLines: 1,
                  maxLines: 6,
                  decoration: InputDecoration(
                    hintText:
                        'Message the AI... (@ to mention files)',
                    filled: true,
                    fillColor: ClawdTheme.surface,
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(10),
                      borderSide:
                          const BorderSide(color: ClawdTheme.surfaceBorder),
                    ),
                    enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(10),
                      borderSide:
                          const BorderSide(color: ClawdTheme.surfaceBorder),
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(10),
                      borderSide: const BorderSide(color: ClawdTheme.claw),
                    ),
                    contentPadding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 8,
                    ),
                  ),
                ),
              ),
            ),
            const SizedBox(width: 8),
            IconButton.filled(
              onPressed: isLoading ? null : _send,
              icon: isLoading
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.arrow_upward, size: 18),
              style: IconButton.styleFrom(
                backgroundColor: ClawdTheme.claw,
                foregroundColor: Colors.white,
                disabledBackgroundColor: ClawdTheme.surfaceBorder,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
