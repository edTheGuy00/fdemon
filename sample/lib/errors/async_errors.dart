// ignore_for_file: dangling_library_doc_comments

/// Asynchronous error triggers for Flutter Demon testing.
///
/// These functions intentionally trigger async exceptions to test
/// error highlighting and async stack trace display.

/// Triggers a simple async error after a delay.
Future<void> triggerAsyncError() async {
  await Future.delayed(const Duration(milliseconds: 100));
  throw Exception('Async exception after delay');
}

/// Triggers a nested async error (3 levels deep).
Future<void> triggerNestedAsyncError() async {
  await _asyncHelper1();
}

Future<void> _asyncHelper1() async {
  await _asyncHelper2();
}

Future<void> _asyncHelper2() async {
  await _asyncHelper3();
}

Future<void> _asyncHelper3() async {
  throw StateError('Nested async error at level 3');
}

/// Triggers a timeout error.
Future<void> triggerTimeoutError() async {
  await Future.delayed(const Duration(seconds: 10))
      .timeout(const Duration(milliseconds: 100));
}

/// Triggers an async error with multiple async suspensions.
Future<void> triggerMultipleAsyncSuspensions() async {
  await Future.delayed(const Duration(milliseconds: 50));
  await _waitAndThrow1();
}

Future<void> _waitAndThrow1() async {
  await Future.delayed(const Duration(milliseconds: 50));
  await _waitAndThrow2();
}

Future<void> _waitAndThrow2() async {
  await Future.delayed(const Duration(milliseconds: 50));
  throw Exception('Error after multiple async suspensions');
}

/// Triggers an uncaught async error (fire and forget).
void triggerUncaughtAsyncError() {
  Future.delayed(const Duration(milliseconds: 100), () {
    throw Exception('Uncaught async error (fire and forget)');
  });
}

/// Triggers a stream error.
Future<void> triggerStreamError() async {
  final stream = Stream<int>.periodic(
    const Duration(milliseconds: 50),
    (count) {
      if (count == 3) throw Exception('Stream error at count 3');
      return count;
    },
  );

  await for (final value in stream) {
    // ignore: avoid_print
    print('Stream value: $value');
  }
}
