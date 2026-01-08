import 'package:flutter/widgets.dart';

void main() {
  // Type error: assigning String to int
  int count = 'not a number';
  debugPrint('Count: $count');
  runApp(const ErrorApp());
}

class ErrorApp extends StatelessWidget {
  const ErrorApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: Text('Type Error App', textDirection: TextDirection.ltr),
    );
  }
}
