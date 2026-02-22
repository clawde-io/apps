import 'package:flutter_test/flutter_test.dart';
import 'package:clawde_mobile/app.dart';

void main() {
  testWidgets('app renders without crashing', (WidgetTester tester) async {
    await tester.pumpWidget(const ClawDEMobileApp());
  });
}
