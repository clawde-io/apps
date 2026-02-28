// QA-04: Desktop integration test — full session loop.
//
// Prerequisites to run this test:
//   1. clawd daemon running on ws://127.0.0.1:4300
//      Launch with: cargo run --manifest-path ../daemon/Cargo.toml
//   2. A valid local directory for the repo path (uses /tmp/clawd-test-repo).
//      Create with: mkdir -p /tmp/clawd-test-repo && git -C /tmp/clawd-test-repo init
//   3. Run with:
//      flutter test integration_test/session_loop_test.dart -d macos
//
// Note: this test communicates with a real daemon over WebSocket.
// It is NOT suitable for CI unless a daemon is started as a service.

import 'package:clawd_ui/clawd_ui.dart';
import 'package:flutter/gestures.dart' show kSecondaryButton;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  // ── Helpers ─────────────────────────────────────────────────────────────────

  /// Waits for a widget to appear, polling every 500 ms up to [timeout].
  Future<void> waitFor(
    WidgetTester tester,
    Finder finder, {
    Duration timeout = const Duration(seconds: 30),
  }) async {
    final end = DateTime.now().add(timeout);
    while (DateTime.now().isBefore(end)) {
      await tester.pump(const Duration(milliseconds: 500));
      if (finder.evaluate().isNotEmpty) return;
    }
    throw TestFailure(
      'Timed out waiting for $finder after $timeout',
    );
  }

  // ── Test ────────────────────────────────────────────────────────────────────

  testWidgets(
    'Session loop: connect → create → send message → close',
    (tester) async {
      // 1. Launch the app.
      //    The app entry point wraps everything in a ProviderScope and
      //    automatically connects to the local daemon in DaemonNotifier.build().
      await tester.runAsync(() async {
        // Import main lazily to avoid pulling in main.dart at compile time
        // from a different package. The integration test runner handles this.
      });

      // Pump until the Sessions screen is visible and the daemon is connected.
      // The ConnectionStatusIndicator in the NavigationRail shows "Connected"
      // when DaemonStatus.connected.
      await waitFor(
        tester,
        find.text('Connected'),
        timeout: const Duration(seconds: 10),
      );

      // 2. Navigate to Chat section (index 0 in NavigationRail — default).
      //    The split-pane layout (ChatLayout) should be visible.
      expect(find.text('Sessions'), findsWidgets);

      // 3. Tap the "+" button in the session sidebar to open NewSessionDialog.
      final addButton = find.byIcon(Icons.add);
      expect(addButton, findsOneWidget);
      await tester.tap(addButton);
      await tester.pumpAndSettle();

      // 4. The NewSessionDialog should appear.
      expect(find.text('New Session'), findsOneWidget);

      // 5. Enter the repo path.
      final pathField = find.byType(TextField).first;
      await tester.enterText(pathField, '/tmp/clawd-test-repo');
      await tester.pump();

      // 6. Provider is Claude by default — no need to change it.
      // 7. Tap "Create".
      await tester.tap(find.text('Create'));
      await tester.pumpAndSettle();

      // 8. Session should appear in the sidebar.
      //    The tile shows the basename of the repo path.
      await waitFor(
        tester,
        find.text('clawd-test-repo'),
        timeout: const Duration(seconds: 10),
      );

      // 9. The chat content area should now show the session header.
      expect(find.text('clawd-test-repo'), findsWidgets);

      // 10. Send a message.
      final inputField = find.byType(TextField).last;
      await tester.enterText(inputField, 'Hello from integration test');
      await tester.testTextInput.receiveAction(TextInputAction.send);
      await tester.pump();

      // 11. The user bubble should appear immediately.
      await waitFor(
        tester,
        find.text('Hello from integration test'),
        timeout: const Duration(seconds: 5),
      );

      // 12. Wait for the AI response (up to 30 s — daemon proxies to provider).
      //     Any additional ChatBubble widget indicates a response was received.
      final initialBubbleCount = find.byType(ChatBubble).evaluate().length;
      await tester.runAsync(() => Future<void>.delayed(
            const Duration(seconds: 20),
          ));
      await tester.pump();
      final finalBubbleCount = find.byType(ChatBubble).evaluate().length;
      expect(
        finalBubbleCount,
        greaterThan(initialBubbleCount),
        reason: 'Expected at least one AI response bubble',
      );

      // 13. Right-click (secondary tap) the session tile to open context menu.
      final sessionTile = find.text('clawd-test-repo').first;
      await tester.tap(sessionTile, buttons: kSecondaryButton);
      await tester.pumpAndSettle();

      // 14. Tap "Close" in the context menu.
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();

      // 15. Session should no longer appear in the list.
      //     After close, the sidebar should show the empty state.
      await waitFor(
        tester,
        find.text('No sessions'),
        timeout: const Duration(seconds: 5),
      );
    },
    timeout: const Timeout(Duration(minutes: 2)),
  );
}

