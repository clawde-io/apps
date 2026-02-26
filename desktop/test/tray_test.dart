// SPDX-License-Identifier: MIT
// Sprint II ST.7 — Tray widget tests.

import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawde/features/tray/tray_manager_widget.dart';

// Note: TrayManagerWidget depends on tray_manager + window_manager which
// are native desktop plugins and cannot be exercised in headless unit tests.
// These tests cover the widget construction and child rendering only.

// ── Fakes ───────────────────────────────────────────────────────────────────

/// Stays disconnected — no WebSocket connection or reconnect timers in tests.
class _FakeDaemonNotifier extends DaemonNotifier {
  @override
  DaemonState build() => const DaemonState();
}

class _FakeSessionListNotifier extends SessionListNotifier {
  @override
  Future<List<Session>> build() async => [];
}

// Overrides shared by every test in this file.
List<Override> _overrides() => [
      bootstrapTokenProvider.overrideWithValue(null),
      daemonProvider.overrideWith(_FakeDaemonNotifier.new),
      sessionListProvider.overrideWith(_FakeSessionListNotifier.new),
    ];

void main() {
  group('TrayManagerWidget', () {
    testWidgets('renders child widget unchanged', (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: _overrides(),
          child: const MaterialApp(
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
          overrides: _overrides(),
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
        ProviderScope(
          overrides: _overrides(),
          child: const MaterialApp(
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
