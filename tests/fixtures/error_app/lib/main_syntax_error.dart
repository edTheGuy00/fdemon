import 'package:flutter/widgets.dart';

void main() {
  // Missing closing brace - syntax error
  runApp(const ErrorApp()
}

class ErrorApp extends StatelessWidget {
  const ErrorApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: Text('Syntax Error App', textDirection: TextDirection.ltr),
    );
  }
}
