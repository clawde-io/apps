// SPDX-License-Identifier: MIT
// Sprint II ST.7 — Tray widget tests.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawde/features/tray/tray_manager_widget.dart';

// Note: TrayManagerWidget depends on tray_manager + window_manager which
// are native desktop plugins and cannot be exercised in headless unit tests.
// These tests cover the widget construction and child rendering only.

void main() {
  group('TrayManagerWidget', () {
    testWidgets('renders child widget unchanged', (tester) async {
      await tester.pumpWidget(
        const ProviderScope(
          child: MaterialApp(
            home: TrayManagerWidget(
              child: Scaffold(
                body: Center(child: Text('hello')),
              ),
            ),
          ),
        ),
      );

      // The child should be rendered as-is.
      expect(find.text('hello'), findsOneWidget);
    });

    testWidgets('accepts optional callbacks without error', (tester) async {
      bool newSessionCalled = false;
      String? navigatedSession;

      await tester.pumpWidget(
        ProviderScope(
          child: MaterialApp(
            home: TrayManagerWidget(
              onNewSession: () => newSessionCalled = true,
              onShowSession: (id) => navigatedSession = id,
              child: const SizedBox.shrink(),
            ),
          ),
        ),
      );

      // Callbacks are stored — verify widget constructed without error.
      expect(newSessionCalled, isFalse);
      expect(navigatedSession, isNull);
    });

    testWidgets('renders without callbacks', (tester) async {
      await tester.pumpWidget(
        const ProviderScope(
          child: MaterialApp(
            home: TrayManagerWidget(
              child: Text('no callbacks'),
            ),
          ),
        ),
      );

      expect(find.text('no callbacks'), findsOneWidget);
    });
  });
}
