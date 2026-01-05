// ignore_for_file: dangling_library_doc_comments

/// Logger package demonstrations for Flutter Demon testing.
///
/// The `logger` package provides pretty-printed logs with customizable
/// formatters, including box-drawing characters and emoji indicators.

import 'package:logger/logger.dart';

/// Logger with pretty printer - includes stack traces and emoji.
final logger = Logger(
  printer: PrettyPrinter(
    methodCount: 5, // Number of method calls in stack trace
    errorMethodCount: 10, // Stack trace depth for errors
    lineLength: 120,
    colors: true,
    printEmojis: true,
    dateTimeFormat: DateTimeFormat.onlyTimeAndSinceStart,
  ),
);

/// Logger without stack traces - cleaner output.
final loggerNoStack = Logger(
  printer: PrettyPrinter(methodCount: 0),
);

/// Simple one-line logger.
final simpleLogger = Logger(
  printer: SimplePrinter(colors: true),
);

/// Demonstrates all Logger log levels.
void demonstrateLoggerPackage() {
  logger.t('Trace: Very detailed debugging info');
  logger.d('Debug: Debugging information');
  logger.i('Info: General information');
  logger.w('Warning: Something might be wrong');
  logger.e('Error: Something went wrong!');
  logger.f('Fatal: Critical failure!');
}

/// Demonstrates Logger with exception and stack trace.
void demonstrateLoggerWithException() {
  try {
    throw FormatException('Invalid format in logger demo');
  } catch (e, st) {
    logger.e('Caught format exception', error: e, stackTrace: st);
  }
}

/// Demonstrates Logger with complex data structures.
void demonstrateLoggerWithData() {
  logger.i({
    'user': 'john_doe',
    'action': 'login',
    'timestamp': DateTime.now().toIso8601String(),
    'metadata': {'ip': '192.168.1.1', 'device': 'iPhone'},
  });
}

/// Demonstrates Logger with multi-line message.
void demonstrateLoggerMultiLine() {
  logger.i('''
Multi-line log message:
- Line 1: First item
- Line 2: Second item
- Line 3: Third item
''');
}

/// Demonstrates the simple one-line printer.
void loggerWithCustomPrinter() {
  simpleLogger.i('Simple one-line log');
  simpleLogger.w('Simple warning');
  simpleLogger.e('Simple error');
}

/// Logs using logger without stack traces.
void loggerWithoutStackTrace() {
  loggerNoStack.i('Info without stack trace');
  loggerNoStack.w('Warning without stack trace');
  loggerNoStack.e('Error without stack trace');
}
