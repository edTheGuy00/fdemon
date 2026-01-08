import 'package:flutter/widgets.dart';
import 'package:core/core.dart';
import 'package:ui_components/ui_components.dart';

void main() {
  debugPrint('[FDEMON_TEST] multi_module main_app starting');
  CoreLogger.log('App initialized');
  runApp(const MainApp());
}

class MainApp extends StatelessWidget {
  const MainApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: AppTitle(text: 'Multi-Module App'),
    );
  }
}
