// ignore_for_file: dangling_library_doc_comments

/// Mixed logger usage demonstrations for Flutter Demon testing.
///
/// This file demonstrates using both Logger and Talker together,
/// producing interleaved output from different logging libraries.

import 'package:logger/logger.dart';
import 'package:talker_flutter/talker_flutter.dart';

final _logger = Logger(
  printer: PrettyPrinter(methodCount: 2),
);

final _talker = TalkerFlutter.init();

/// Demonstrates alternating between Logger and Talker.
void demonstrateMixedLoggers() {
  _logger.i('Logger: Starting mixed logging demo');
  _talker.info('Talker: Starting mixed logging demo');

  _logger.d('Logger: Debug message');
  _talker.debug('Talker: Debug message');

  _logger.w('Logger: Warning message');
  _talker.warning('Talker: Warning message');

  _logger.e('Logger: Error message');
  _talker.error('Talker: Error message');

  _logger.i('Logger: Mixed demo complete');
  _talker.info('Talker: Mixed demo complete');
}

/// Simulates a request flow with mixed loggers.
void simulateRequestFlow() {
  _talker.info('Request: POST /api/users');
  _logger.d({'method': 'POST', 'path': '/api/users', 'body': {'name': 'test'}});

  Future.delayed(const Duration(milliseconds: 200), () {
    _logger.i('Processing request...');
    _talker.debug('Validating input data');
  });

  Future.delayed(const Duration(milliseconds: 400), () {
    _talker.info('Response: 201 Created');
    _logger.i({'status': 201, 'id': 42});
  });
}

/// Logs verbose output to test scrolling and performance.
void verboseLogging(int count) {
  for (int i = 0; i < count; i++) {
    if (i % 2 == 0) {
      _logger.d('Logger verbose message #$i with some extra text to make it longer');
    } else {
      _talker.verbose('Talker verbose message #$i with additional details');
    }
  }
}
