// SPDX-License-Identifier: MIT
// Editor unit tests (Sprint HH, ED.14).

import 'package:flutter_test/flutter_test.dart';
import 'package:clawde/features/editor/editor_tab_bar.dart';
import 'package:clawde/features/editor/js_bridge.dart';

void main() {
  group('EditorTabController', () {
    test('opens a new tab and sets it active', () {
      final ctrl = EditorTabController();
      ctrl.openFile(path: 'lib/main.dart', language: 'dart', content: 'void main() {}');
      expect(ctrl.tabs.length, 1);
      expect(ctrl.activeIndex, 0);
      expect(ctrl.activeTab?.fileName, 'main.dart');
    });

    test('switching to an existing path reuses the tab', () {
      final ctrl = EditorTabController();
      ctrl.openFile(path: 'lib/main.dart', language: 'dart');
      ctrl.openFile(path: 'lib/app.dart', language: 'dart');
      ctrl.openFile(path: 'lib/main.dart', language: 'dart'); // duplicate
      expect(ctrl.tabs.length, 2);
      expect(ctrl.activeIndex, 0); // switched back to first
    });

    test('close removes tab and adjusts active index', () {
      final ctrl = EditorTabController();
      ctrl.openFile(path: 'a.dart', language: 'dart');
      ctrl.openFile(path: 'b.dart', language: 'dart');
      ctrl.openFile(path: 'c.dart', language: 'dart');
      ctrl.closeTab(1); // remove b.dart
      expect(ctrl.tabs.length, 2);
      expect(ctrl.tabs.map((t) => t.fileName).toList(), ['a.dart', 'c.dart']);
    });

    test('closing all tabs sets activeIndex to -1', () {
      final ctrl = EditorTabController();
      ctrl.openFile(path: 'x.dart', language: 'dart');
      ctrl.closeTab(0);
      expect(ctrl.tabs.isEmpty, true);
      expect(ctrl.activeIndex, -1);
      expect(ctrl.activeTab, isNull);
    });

    test('markDirty sets isDirty flag', () {
      final ctrl = EditorTabController();
      ctrl.openFile(path: 'dirty.dart', language: 'dart');
      expect(ctrl.tabs[0].isDirty, false);
      ctrl.markDirty(0);
      expect(ctrl.tabs[0].isDirty, true);
    });

    test('updateContent sets content and marks dirty', () {
      final ctrl = EditorTabController();
      ctrl.openFile(path: 'f.dart', language: 'dart', content: '// old');
      ctrl.updateContent(0, '// new');
      expect(ctrl.tabs[0].content, '// new');
      expect(ctrl.tabs[0].isDirty, true);
    });
  });

  group('EditorEvent.fromJson', () {
    test('parses change event', () {
      final event = EditorEvent.fromJson({'type': 'change', 'content': 'fn main() {}'});
      expect(event.type, 'change');
      expect(event.content, 'fn main() {}');
    });

    test('parses cursorMove event', () {
      final event = EditorEvent.fromJson({'type': 'cursorMove', 'cursorLine': 5, 'cursorCol': 12});
      expect(event.cursorLine, 5);
      expect(event.cursorCol, 12);
    });

    test('unknown type does not throw', () {
      final event = EditorEvent.fromJson({'type': 'unknown'});
      expect(event.type, 'unknown');
    });

    test('missing fields default to null', () {
      final event = EditorEvent.fromJson({'type': 'ready'});
      expect(event.content, isNull);
      expect(event.path, isNull);
    });
  });
}
