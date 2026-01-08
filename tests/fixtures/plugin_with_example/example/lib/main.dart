import 'package:flutter/widgets.dart';
import 'package:test_plugin/test_plugin.dart';

void main() {
  debugPrint('[FDEMON_TEST] plugin_example starting');
  TestPlugin.logMessage('Plugin loaded');
  runApp(const PluginExampleApp());
}

class PluginExampleApp extends StatelessWidget {
  const PluginExampleApp({super.key});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Text(
        'Plugin Example: ${TestPlugin.platformVersion}',
        textDirection: TextDirection.ltr,
        style: const TextStyle(fontSize: 20),
      ),
    );
  }
}
