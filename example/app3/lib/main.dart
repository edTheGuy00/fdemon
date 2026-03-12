import 'package:flutter/material.dart';

void main() {
  runApp(const App3());
}

/// Minimal Flutter app for fdemon multi-config auto_start testing.
///
/// When run via `cargo run -- example/app3`, fdemon should automatically
/// launch with the "Staging" configuration because its launch.toml has
/// `auto_start = true` on that config entry.
class App3 extends StatelessWidget {
  const App3({super.key});

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(
      title: 'Flutter Demon App 3',
      home: Scaffold(
        body: Center(child: Text('App 3 — multi-config auto_start fixture')),
      ),
    );
  }
}
