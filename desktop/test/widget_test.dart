// CK-02: Desktop widget tests — SessionSidebar rendering and interaction.
// MI.T30: UsageDashboardScreen widget tests.
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:clawde/app.dart';
import 'package:clawde/features/chat/widgets/session_sidebar.dart';
import 'package:clawde/features/usage/usage_dashboard_screen.dart';

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

Widget _wrapSidebar({
  List<Session> sessions = const [],
  VoidCallback? onNewSession,
}) =>
    ProviderScope(
      overrides: [
        bootstrapTokenProvider.overrideWithValue(null),
        daemonProvider.overrideWith(_FakeDaemonNotifier.new),
        sessionListProvider.overrideWith(
          () => _FakeSessionListNotifier(sessions),
        ),
      ],
      child: MaterialApp(
        home: Scaffold(body: SessionSidebar(onNewSession: onNewSession)),
      ),
    );

// ── Tests ─────────────────────────────────────────────────────────────────────

void main() {
  testWidgets('app renders without crashing', (WidgetTester tester) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          bootstrapTokenProvider.overrideWithValue(null),
          daemonProvider.overrideWith(_FakeDaemonNotifier.new),
          sessionListProvider.overrideWith(
            () => _FakeSessionListNotifier([]),
          ),
        ],
        child: const ClawDEApp(),
      ),
    );
  });

  // ── SessionSidebar ──────────────────────────────────────────────────────────

  group('SessionSidebar', () {
    testWidgets('shows Sessions header', (tester) async {
      await tester.pumpWidget(_wrapSidebar());
      await tester.pump();
      expect(find.text('Sessions'), findsOneWidget);
    });

    testWidgets('shows + button', (tester) async {
      await tester.pumpWidget(_wrapSidebar());
      await tester.pump();
      expect(find.byIcon(Icons.add), findsOneWidget);
    });

    testWidgets('calls onNewSession when + button is tapped', (tester) async {
      var called = false;
      await tester.pumpWidget(_wrapSidebar(onNewSession: () => called = true));
      await tester.pump();
      await tester.tap(find.byIcon(Icons.add));
      await tester.pump();
      expect(called, isTrue);
    });

    testWidgets('shows empty state when no sessions', (tester) async {
      await tester.pumpWidget(_wrapSidebar());
      await tester.pump();
      expect(find.text('No sessions'), findsOneWidget);
      expect(find.text('Tap + to start an AI session'), findsOneWidget);
    });

    testWidgets('shows session tiles when sessions exist', (tester) async {
      final sessions = [
        _session(id: 's1', repoPath: '/projects/alpha'),
        _session(id: 's2', repoPath: '/projects/beta'),
      ];
      await tester.pumpWidget(_wrapSidebar(sessions: sessions));
      await tester.pump();
      // SessionListTile renders repoPath.split('/').last as its title.
      expect(find.text('alpha'), findsOneWidget);
      expect(find.text('beta'), findsOneWidget);
    });

    testWidgets('selecting a session updates activeSessionIdProvider',
        (tester) async {
      final sessions = [
        _session(id: 's1', repoPath: '/projects/alpha'),
      ];
      final container = ProviderContainer(
        overrides: [
          bootstrapTokenProvider.overrideWithValue(null),
          daemonProvider.overrideWith(_FakeDaemonNotifier.new),
          sessionListProvider.overrideWith(
            () => _FakeSessionListNotifier(sessions),
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: SessionSidebar()),
          ),
        ),
      );
      await tester.pump();

      // Tap the session tile.
      await tester.tap(find.text('alpha'));
      await tester.pump();

      expect(container.read(activeSessionIdProvider), 's1');
    });
  });

  // ── UsageDashboardScreen — MI.T30 ─────────────────────────────────────────

  Widget wrapDashboard({
    Map<String, dynamic>? budget,
    List<Map<String, dynamic>> modelUsage = const [],
    List<Session> sessions = const [],
  }) =>
      ProviderScope(
        overrides: [
          bootstrapTokenProvider.overrideWithValue(null),
          daemonProvider.overrideWith(_FakeDaemonNotifier.new),
          sessionListProvider.overrideWith(
            () => _FakeSessionListNotifier(sessions),
          ),
          tokenBudgetStatusProvider.overrideWith((ref) async => budget),
          tokenTotalUsageProvider.overrideWith((ref) async => modelUsage),
          tokenSessionUsageProvider.overrideWith((ref, _) async => null),
        ],
        child: const MaterialApp(
          home: UsageDashboardScreen(),
        ),
      );

  group('UsageDashboardScreen', () {
    testWidgets('renders Usage Dashboard title in app bar', (tester) async {
      await tester.pumpWidget(wrapDashboard());
      await tester.pumpAndSettle();

      expect(find.text('Usage Dashboard'), findsOneWidget);
    });

    testWidgets('shows empty-month state when no model usage recorded',
        (tester) async {
      await tester.pumpWidget(wrapDashboard(modelUsage: []));
      await tester.pumpAndSettle();

      expect(find.text('No usage recorded this month'), findsOneWidget);
    });

    testWidgets('renders model breakdown table with data', (tester) async {
      await tester.pumpWidget(wrapDashboard(
        modelUsage: [
          {
            'modelId': 'claude-sonnet-4-6',
            'inputTokens': 5000,
            'outputTokens': 2000,
            'estimatedCostUsd': 0.045,
            'messageCount': 3,
          },
        ],
      ));
      await tester.pumpAndSettle();

      // Table column header + shortened model name.
      expect(find.text('Model'), findsOneWidget);
      expect(find.text('Claude Sonnet'), findsOneWidget);
    });

    testWidgets('shows spend amount in budget card (no cap)', (tester) async {
      await tester.pumpWidget(wrapDashboard(
        budget: {
          'monthlySpendUsd': 3.1234,
          'cap': null,
          'pct': null,
          'warning': false,
          'exceeded': false,
        },
      ));
      await tester.pumpAndSettle();

      expect(find.textContaining('3.1234'), findsOneWidget);
    });

    testWidgets('shows Within budget badge when within cap', (tester) async {
      await tester.pumpWidget(wrapDashboard(
        budget: {
          'monthlySpendUsd': 2.0,
          'cap': 10.0,
          'pct': 20.0,
          'warning': false,
          'exceeded': false,
        },
      ));
      await tester.pumpAndSettle();

      expect(find.text('Within budget'), findsOneWidget);
    });

    testWidgets('shows Budget warning badge when warning=true', (tester) async {
      await tester.pumpWidget(wrapDashboard(
        budget: {
          'monthlySpendUsd': 8.5,
          'cap': 10.0,
          'pct': 85.0,
          'warning': true,
          'exceeded': false,
        },
      ));
      await tester.pumpAndSettle();

      expect(find.text('Budget warning'), findsOneWidget);
    });

    testWidgets('shows Budget exceeded badge when exceeded=true', (tester) async {
      await tester.pumpWidget(wrapDashboard(
        budget: {
          'monthlySpendUsd': 12.0,
          'cap': 10.0,
          'pct': 120.0,
          'warning': true,
          'exceeded': true,
        },
      ));
      await tester.pumpAndSettle();

      expect(find.text('Budget exceeded'), findsOneWidget);
    });

    testWidgets('shows no-sessions message when session list is empty',
        (tester) async {
      await tester.pumpWidget(wrapDashboard(sessions: []));
      await tester.pumpAndSettle();

      expect(find.text('No sessions found'), findsOneWidget);
    });

    testWidgets('shows repo name in per-session cost table', (tester) async {
      final sess = _session(
        id: 's-dash',
        repoPath: '/projects/widget-test',
      );
      await tester.pumpWidget(wrapDashboard(sessions: [sess]));
      await tester.pumpAndSettle();

      // Per-session table header and the repo name.
      expect(find.text('Session'), findsOneWidget);
      expect(find.text('widget-test'), findsOneWidget);
    });
  });
}
