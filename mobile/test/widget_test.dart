import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:clawde_mobile/app.dart';

/// Stays disconnected — no WebSocket connection or reconnect timers in tests.
class _FakeDaemonNotifier extends DaemonNotifier {
  @override
  DaemonState build() => const DaemonState();
}

/// Returns an empty session list immediately — no WebSocket calls in tests.
class _FakeSessionListNotifier extends SessionListNotifier {
  @override
  Future<List<Session>> build() async => [];
}

void main() {
  testWidgets('app renders without crashing', (WidgetTester tester) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          daemonProvider.overrideWith(_FakeDaemonNotifier.new),
          sessionListProvider.overrideWith(_FakeSessionListNotifier.new),
        ],
        child: const ClawDEMobileApp(),
      ),
    );
  });
}
