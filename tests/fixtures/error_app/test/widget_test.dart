import 'package:flutter_test/flutter_test.dart';
import 'package:error_app/main.dart';

void main() {
  testWidgets('ErrorApp smoke test', (WidgetTester tester) async {
    // Build our app and trigger a frame.
    await tester.pumpWidget(const ErrorApp());

    // Verify that the app renders without crashing.
    expect(find.text('Error App - Working Mode'), findsOneWidget);
  });
}
