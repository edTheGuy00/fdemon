// ignore_for_file: dangling_library_doc_comments

/// Flutter-specific error scenarios for Flutter Demon testing.
///
/// These functions trigger various Flutter framework errors
/// to test error rendering and Flutter-specific stack traces.

import 'package:flutter/material.dart';

/// Creates a widget that will cause a RenderFlex overflow.
class OverflowWidget extends StatelessWidget {
  const OverflowWidget({super.key});

  @override
  Widget build(BuildContext context) {
    return Row(
      children: List.generate(
        50,
        (i) => Container(width: 100, height: 100, color: Colors.red),
      ),
    );
  }
}

/// Creates a widget that triggers setState after dispose.
class LeakyWidget extends StatefulWidget {
  const LeakyWidget({super.key});

  @override
  State<LeakyWidget> createState() => _LeakyWidgetState();
}

class _LeakyWidgetState extends State<LeakyWidget> {
  @override
  void initState() {
    super.initState();
    Future.delayed(const Duration(seconds: 2), () {
      if (!mounted) {
        // This would error in real scenarios, but we check mounted
        debugPrint('Would have called setState after dispose');
      }
    });
  }

  @override
  Widget build(BuildContext context) => const SizedBox();
}

/// Throws an error during build.
class BuildErrorWidget extends StatelessWidget {
  const BuildErrorWidget({super.key});

  @override
  Widget build(BuildContext context) {
    throw FlutterError('Intentional build error for testing');
  }
}

/// Triggers a layout error.
void triggerLayoutError(BuildContext context) {
  // Attempt to use context after disposal
  Future.delayed(const Duration(milliseconds: 100), () {
    try {
      // ignore: use_build_context_synchronously
      MediaQuery.of(context);
    } catch (e) {
      debugPrint('Layout error caught: $e');
    }
  });
}

/// Triggers a general Flutter error.
void triggerFlutterError() {
  throw FlutterError.fromParts([
    ErrorSummary('Intentional Flutter error'),
    ErrorDescription('This error was triggered for testing purposes.'),
    ErrorHint('Check Flutter Demon log view for stack trace rendering.'),
  ]);
}

/// Triggers a Flutter assertion error.
void triggerFlutterAssertion() {
  assert(false, 'Intentional Flutter assertion failure');
}

/// Creates a platform exception scenario.
void triggerPlatformException() {
  throw Exception('Simulated platform channel error: Method not found');
}
