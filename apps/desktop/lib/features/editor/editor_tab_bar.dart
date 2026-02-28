// SPDX-License-Identifier: MIT
// Multi-tab editor widget (Sprint HH, ED.4).

import 'package:flutter/material.dart';

// ─── EditorTab ────────────────────────────────────────────────────────────────

/// Represents a single open file tab in the editor.
class EditorTab {
  EditorTab({required this.path, required this.language, String? content})
      : content = content ?? '';

  final String path;
  final String language;
  String content;
  bool isDirty = false;

  String get fileName {
    final parts = path.replaceAll('\\', '/').split('/');
    return parts.last;
  }
}

// ─── EditorTabController ──────────────────────────────────────────────────────

/// Manages the list of open tabs and the active tab index.
class EditorTabController extends ChangeNotifier {
  final List<EditorTab> _tabs = [];
  int _activeIndex = -1;

  List<EditorTab> get tabs => List.unmodifiable(_tabs);
  EditorTab? get activeTab => _activeIndex >= 0 && _activeIndex < _tabs.length ? _tabs[_activeIndex] : null;
  int get activeIndex => _activeIndex;

  /// Open a file. If it's already open, switch to it.
  void openFile({required String path, required String language, String content = ''}) {
    final existing = _tabs.indexWhere((t) => t.path == path);
    if (existing >= 0) {
      _activeIndex = existing;
    } else {
      _tabs.add(EditorTab(path: path, language: language, content: content));
      _activeIndex = _tabs.length - 1;
    }
    notifyListeners();
  }

  void closeTab(int index) {
    if (index < 0 || index >= _tabs.length) return;
    _tabs.removeAt(index);
    if (_tabs.isEmpty) {
      _activeIndex = -1;
    } else if (_activeIndex >= _tabs.length) {
      _activeIndex = _tabs.length - 1;
    }
    notifyListeners();
  }

  void setActive(int index) {
    if (index < 0 || index >= _tabs.length) return;
    _activeIndex = index;
    notifyListeners();
  }

  void markDirty(int index, {bool dirty = true}) {
    if (index < 0 || index >= _tabs.length) return;
    _tabs[index].isDirty = dirty;
    notifyListeners();
  }

  void updateContent(int index, String content) {
    if (index < 0 || index >= _tabs.length) return;
    _tabs[index].content = content;
    _tabs[index].isDirty = true;
    notifyListeners();
  }
}

// ─── EditorTabBar ─────────────────────────────────────────────────────────────

/// Horizontal tab bar for the multi-tab editor.
class EditorTabBar extends StatelessWidget {
  const EditorTabBar({
    super.key,
    required this.controller,
  });

  final EditorTabController controller;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        if (controller.tabs.isEmpty) return const SizedBox.shrink();
        return Container(
          height: 36,
          color: const Color(0xFF0d0d12),
          child: ListView.builder(
            scrollDirection: Axis.horizontal,
            itemCount: controller.tabs.length,
            itemBuilder: (context, i) => _Tab(
              tab: controller.tabs[i],
              isActive: i == controller.activeIndex,
              onTap: () => controller.setActive(i),
              onClose: () => controller.closeTab(i),
            ),
          ),
        );
      },
    );
  }
}

class _Tab extends StatelessWidget {
  const _Tab({
    required this.tab,
    required this.isActive,
    required this.onTap,
    required this.onClose,
  });

  final EditorTab tab;
  final bool isActive;
  final VoidCallback onTap;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        constraints: const BoxConstraints(minWidth: 80, maxWidth: 200),
        padding: const EdgeInsets.symmetric(horizontal: 12),
        decoration: BoxDecoration(
          color: isActive ? const Color(0xFF1a1a1f) : Colors.transparent,
          border: Border(
            bottom: BorderSide(
              color: isActive ? const Color(0xFFdc2626) : Colors.transparent,
              width: 2,
            ),
          ),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (tab.isDirty)
              const Padding(
                padding: EdgeInsets.only(right: 4),
                child: CircleAvatar(radius: 3, backgroundColor: Color(0xFFdc2626)),
              ),
            Flexible(
              child: Text(
                tab.fileName,
                style: TextStyle(
                  fontSize: 12,
                  color: isActive ? Colors.white : const Color(0xFF9ca3af),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ),
            const SizedBox(width: 6),
            GestureDetector(
              onTap: onClose,
              child: const Icon(Icons.close, size: 14, color: Color(0xFF6b7280)),
            ),
          ],
        ),
      ),
    );
  }
}
