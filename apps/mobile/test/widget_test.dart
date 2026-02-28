// CK-03: Mobile widget tests — SessionsScreen rendering and state.
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:clawde_mobile/app.dart';
import 'package:clawde_mobile/features/sessions/sessions_screen.dart';

// ── Fakes ──────────────────────────────────────────────────────────────────────

/// Stays disconnected — no WebSocket connection or reconnect timers in tests.
class _FakeDaemonNotifier extends DaemonNotifier {
  @override
  DaemonState build() => const DaemonState();
}

class _FakeSessionListNotifier extends SessionListNotifier {
  _FakeSessionListNotifier(this._sessions);
  final List<Session> _sessions;

  @override
  Future<List<Session>> build() async => _sessions;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

Session _session({
  String id = 's1',
  String repoPath = '/tmp/repo',
  SessionStatus status = SessionStatus.idle,
}) =>
    Session(
      id: id,
      repoPath: repoPath,
      title: '',
      provider: ProviderType.claude,
      status: status,
      createdAt: DateTime(2026),
      updatedAt: DateTime(2026),
      messageCount: 0,
    );

/// Wraps [SessionsScreen] with a minimal GoRouter (required for context.push).
Widget _wrapSessionsScreen(List<Session> sessions) {
  final router = GoRouter(
    routes: [
      GoRoute(
        path: '/',
        builder: (_, __) => const SessionsScreen(),
      ),
      GoRoute(
        path: '/session/:id',
        builder: (_, state) => Scaffold(
          body: Text('session ${state.pathParameters['id']}'),
        ),
      ),
    ],
  );

  return ProviderScope(
    overrides: [
      daemonProvider.overrideWith(_FakeDaemonNotifier.new),
      sessionListProvider.overrideWith(
        () => _FakeSessionListNotifier(sessions),
      ),
    ],
    child: MaterialApp.router(routerConfig: router),
  );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

void main() {
  setUp(() {
    SharedPreferences.setMockInitialValues({});
  });

  testWidgets('app renders without crashing', (WidgetTester tester) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          daemonProvider.overrideWith(_FakeDaemonNotifier.new),
          sessionListProvider.overrideWith(
            () => _FakeSessionListNotifier([]),
          ),
        ],
        child: const ClawDEMobileApp(),
      ),
    );
  });

  // ── SessionsScreen ──────────────────────────────────────────────────────────

  group('SessionsScreen', () {
    testWidgets('shows ClawDE title in app bar', (tester) async {
      await tester.pumpWidget(_wrapSessionsScreen([]));
      await tester.pump();
      expect(find.text('ClawDE'), findsOneWidget);
    });

    testWidgets('shows FAB with add icon', (tester) async {
      await tester.pumpWidget(_wrapSessionsScreen([]));
      await tester.pump();
      expect(find.byIcon(Icons.add), findsOneWidget);
    });

    testWidgets('shows empty state when no sessions', (tester) async {
      await tester.pumpWidget(_wrapSessionsScreen([]));
      await tester.pump();
      expect(find.text('No sessions yet'), findsOneWidget);
      expect(find.text('Tap + to start an AI session'), findsOneWidget);
    });

    testWidgets('shows session tiles when sessions exist', (tester) async {
      final sessions = [
        _session(id: 's1', repoPath: '/projects/alpha'),
        _session(id: 's2', repoPath: '/projects/beta'),
      ];
      await tester.pumpWidget(_wrapSessionsScreen(sessions));
      await tester.pump();
      // SessionListTile renders repoPath.split('/').last as its title.
      expect(find.text('alpha'), findsOneWidget);
      expect(find.text('beta'), findsOneWidget);
    });

    testWidgets('filter chips show correct total count', (tester) async {
      final sessions = [
        _session(id: 's1', status: SessionStatus.running),
        _session(id: 's2', status: SessionStatus.idle),
      ];
      await tester.pumpWidget(_wrapSessionsScreen(sessions));
      await tester.pump();
      expect(find.text('All (2)'), findsOneWidget);
    });

    testWidgets('Running filter chip shows correct count', (tester) async {
      final sessions = [
        _session(id: 's1', status: SessionStatus.running),
        _session(id: 's2', status: SessionStatus.idle),
      ];
      await tester.pumpWidget(_wrapSessionsScreen(sessions));
      await tester.pump();
      expect(find.text('Running (1)'), findsOneWidget);
      expect(find.text('Paused (0)'), findsOneWidget);
    });

    testWidgets('shows connection status indicator', (tester) async {
      await tester.pumpWidget(_wrapSessionsScreen([]));
      await tester.pump();
      // Disconnected daemon shows "Offline – tap" label.
      expect(find.text('Offline – tap'), findsOneWidget);
    });
  });
}
