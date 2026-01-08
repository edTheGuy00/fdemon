import 'package:flutter_test/flutter_test.dart';

import 'package:simple_app/main.dart';

void main() {
  testWidgets('SimpleApp displays text', (WidgetTester tester) async {
    // Build the app
    await tester.pumpWidget(const SimpleApp());

    // Verify the text is displayed
    expect(find.text('Hello from simple_app'), findsOneWidget);
  });
}
