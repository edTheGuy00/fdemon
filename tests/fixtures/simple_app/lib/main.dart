import 'package:flutter/widgets.dart';

void main() {
  // Log message that E2E tests can verify
  debugPrint('[FDEMON_TEST] App starting');
  runApp(const SimpleApp());
}

class SimpleApp extends StatelessWidget {
  const SimpleApp({super.key});

  @override
  Widget build(BuildContext context) {
    debugPrint('[FDEMON_TEST] Building SimpleApp');
    return const Center(
      child: Text(
        'Hello from simple_app',
        textDirection: TextDirection.ltr,
        style: TextStyle(fontSize: 24),
      ),
    );
  }
}
