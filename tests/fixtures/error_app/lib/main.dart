import 'package:flutter/widgets.dart';

void main() {
  debugPrint('[FDEMON_TEST] error_app starting (working mode)');
  runApp(const ErrorApp());
}

class ErrorApp extends StatelessWidget {
  const ErrorApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: Text(
        'Error App - Working Mode',
        textDirection: TextDirection.ltr,
      ),
    );
  }
}
