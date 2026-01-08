import 'package:flutter/widgets.dart';

void main() {
  debugPrint('[FDEMON_TEST] error_app starting (widget error mode)');
  runApp(const WidgetErrorApp());
}

class WidgetErrorApp extends StatelessWidget {
  const WidgetErrorApp({super.key});

  @override
  Widget build(BuildContext context) {
    // Throw during build
    throw FlutterError('Intentional widget build error for testing');
  }
}
