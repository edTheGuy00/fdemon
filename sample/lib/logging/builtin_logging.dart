// ignore_for_file: dangling_library_doc_comments, avoid_print

/// Built-in Dart logging demonstrations for Flutter Demon testing.
///
/// This file demonstrates the standard Dart/Flutter logging mechanisms:
/// - print() - Standard output
/// - debugPrint() - Rate-limited debug output
/// - dart:developer log() - Structured logging with levels

import 'dart:developer' as developer;
import 'package:flutter/foundation.dart';

/// Demonstrates all built-in Dart logging methods.
void demonstrateBuiltInLogging() {
  // Standard print (appears as Info level)
  print('Standard print message');

  // debugPrint (rate-limited, better for large output)
  debugPrint('Debug print message');

  // dart:developer log with various levels
  // Level values: 0=debug, 800=info, 900=warning, 1000=error, 1200=shout
  developer.log('Debug level message', level: 0, name: 'BuiltIn');
  developer.log('Info level message', level: 800, name: 'BuiltIn');
  developer.log('Warning level message', level: 900, name: 'BuiltIn');
  developer.log('Error level message', level: 1000, name: 'BuiltIn');
  developer.log('Shout level message', level: 1200, name: 'BuiltIn');
}

/// Demonstrates logging with additional metadata.
void demonstrateLogWithMetadata() {
  developer.log(
    'Operation completed',
    name: 'BuiltIn.Metadata',
    level: 800,
    time: DateTime.now(),
    sequenceNumber: 42,
  );
}

/// Demonstrates logging an error with stack trace.
void demonstrateLogWithError() {
  try {
    throw Exception('Intentional error for logging demo');
  } catch (e, stackTrace) {
    developer.log(
      'Caught exception in demo',
      name: 'BuiltIn.Error',
      level: 1000,
      error: e,
      stackTrace: stackTrace,
    );
  }
}

/// Logs multiple messages rapidly to test log view performance.
void demonstrateRapidLogging(int count) {
  for (int i = 0; i < count; i++) {
    developer.log(
      'Rapid log message #$i',
      name: 'BuiltIn.Rapid',
      level: 800,
    );
  }
}
