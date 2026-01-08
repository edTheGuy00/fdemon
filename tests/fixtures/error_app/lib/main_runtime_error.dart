import 'package:flutter/widgets.dart';

void main() {
  debugPrint('[FDEMON_TEST] error_app starting (runtime error mode)');
  runApp(const RuntimeErrorApp());
}

class RuntimeErrorApp extends StatelessWidget {
  const RuntimeErrorApp({super.key});

  @override
  Widget build(BuildContext context) {
    // Trigger null dereference
    String? nullValue;
    return Center(
      child: Text(
        nullValue!.toUpperCase(),  // Will throw
        textDirection: TextDirection.ltr,
      ),
    );
  }
}
