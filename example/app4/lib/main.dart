import 'package:flutter/material.dart';

void main() {
  runApp(const App4());
}

/// Minimal Flutter app for fdemon watcher path edge cases testing.
///
/// When run via `cargo run -- example/app4`, fdemon should watch:
///   - example/app4/lib/          (own lib)
///   - example/shared_lib/        (resolved from "../../shared_lib")
///   - example/app1/lib/          (resolved from "../app1/lib")
///
/// Editing any .dart or .json file in those directories should trigger
/// hot reload.
class App4 extends StatelessWidget {
  const App4({super.key});

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(
      title: 'Flutter Demon App 4',
      home: Scaffold(
        body: Center(child: Text('App 4 — watcher path edge cases fixture')),
      ),
    );
  }
}
