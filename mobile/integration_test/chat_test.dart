// QA-05: Mobile integration test — session create and send message.
//
// Prerequisites to run this test:
//   1. clawd daemon running on ws://127.0.0.1:4300 (on the same machine as the
//      device/emulator, or reachable via the configured host URL).
//   2. A valid git repository exists at /tmp/clawd-test-repo on the daemon host.
//      Create with: mkdir -p /tmp/clawd-test-repo && git -C /tmp/clawd-test-repo init
//   3. On a real device, configure the daemon host URL in the Hosts screen first,
//      or run on an emulator where localhost resolves to the host machine.
//   4. Run with:
//      flutter test integration_test/chat_test.dart -d <device-id>
//
// Note: this test communicates with a real daemon over WebSocket.

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
    throw TestFailure('Timed out waiting for $finder after $timeout');
  }

  // ── Test ────────────────────────────────────────────────────────────────────

  testWidgets(
    'Mobile chat: sessions screen → create session → send message',
    (tester) async {
      // 1. App launches and shows the Sessions screen (first tab).
      //    The bottom nav bar has Sessions | Hosts | Settings.
      await waitFor(
        tester,
        find.text('Sessions'),
        timeout: const Duration(seconds: 10),
      );

      // The Sessions tab should be selected (index 0).
      expect(find.byIcon(Icons.chat_bubble), findsWidgets);

      // 2. Either an empty state or an existing session list is shown.
      //    Acceptable initial state: empty-state text OR one or more session tiles.
      final hasEmptyState = find.text('No sessions yet').evaluate().isNotEmpty ||
          find.text('No sessions').evaluate().isNotEmpty;
      final hasSessionList = find.byType(ListTile).evaluate().isNotEmpty;
      expect(hasEmptyState || hasSessionList, isTrue,
          reason: 'Expected sessions screen to show either empty state or list');

      // 3. Tap the "+" FAB to open the New Session sheet.
      final fab = find.byType(FloatingActionButton);
      expect(fab, findsOneWidget);
      await tester.tap(fab);
      await tester.pumpAndSettle();

      // 4. The new session bottom sheet / dialog should be visible.
      expect(find.text('New Session'), findsOneWidget);

      // 5. Enter the repo path in the first text field.
      final repoField = find.byType(TextField).first;
      await tester.enterText(repoField, '/tmp/clawd-test-repo');
      await tester.pump();

      // 6. Tap "Create" (or equivalent confirm button).
      final createButton = find.text('Create');
      expect(createButton, findsOneWidget);
      await tester.tap(createButton);
      await tester.pumpAndSettle();

      // 7. A new session tile should appear in the sessions list.
      await waitFor(
        tester,
        find.text('clawd-test-repo'),
        timeout: const Duration(seconds: 10),
      );

      // 8. Tap the new session tile to open the session detail screen.
      await tester.tap(find.text('clawd-test-repo').first);
      await tester.pumpAndSettle();

      // 9. Session detail screen should be visible — the message input bar
      //    is present at the bottom of the screen.
      await waitFor(
        tester,
        find.byType(TextField),
        timeout: const Duration(seconds: 5),
      );

      // 10. Type a message and tap Send.
      const testMessage = 'Hello from mobile integration test';
      await tester.enterText(find.byType(TextField).last, testMessage);
      await tester.pump();

      // Tap the send icon button.
      await tester.tap(find.byIcon(Icons.arrow_upward));
      await tester.pump();

      // 11. The user's message bubble should appear immediately.
      await waitFor(
        tester,
        find.text(testMessage),
        timeout: const Duration(seconds: 5),
      );

      // 12. Navigate back to Sessions screen.
      final backButton = find.byType(BackButton);
      if (backButton.evaluate().isNotEmpty) {
        await tester.tap(backButton);
      } else {
        await tester.tap(find.byIcon(Icons.arrow_back));
      }
      await tester.pumpAndSettle();

      // 13. Sessions screen is visible again.
      expect(find.text('Sessions'), findsWidgets);
    },
    timeout: const Timeout(Duration(minutes: 3)),
  );
}
