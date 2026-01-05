// ignore_for_file: dangling_library_doc_comments

/// Talker package demonstrations for Flutter Demon testing.
///
/// The `talker` package provides structured logging with built-in
/// exception handling and a visual log viewer (TalkerScreen).

import 'package:talker_flutter/talker_flutter.dart';

/// Global Talker instance for the sample app.
final talker = TalkerFlutter.init(
  settings: TalkerSettings(
    useHistory: true,
    useConsoleLogs: true,
    maxHistoryItems: 1000,
  ),
);

/// Demonstrates all Talker log levels.
void demonstrateTalkerPackage() {
  talker.verbose('Verbose: Maximum detail');
  talker.debug('Debug: Debugging info');
  talker.info('Info: General information');
  talker.warning('Warning: Potential issue');
  talker.error('Error: Something failed');
  talker.critical('Critical: System failure');
  // Note: 'good' level doesn't exist, using info for success messages
  talker.info('Success: Operation completed!');
}

/// Demonstrates Talker exception handling with context.
void demonstrateTalkerWithException() {
  try {
    throw Exception('Talker exception test');
  } catch (e, st) {
    talker.handle(e, st, 'Context: User login failed');
  }
}

/// Demonstrates Talker error handling.
void demonstrateTalkerWithError() {
  try {
    throw StateError('Invalid state for operation');
  } catch (e, st) {
    talker.handle(e, st);
  }
}

/// Demonstrates Talker custom log.
void demonstrateTalkerTypedLog() {
  // Using standard log method for custom messages
  talker.info('Custom log message via info level');
  talker.debug('Custom log message via debug level');
}

/// Simulates HTTP request/response logging with Talker.
void talkerHttpSimulation() {
  talker.info('HTTP Request: GET /api/users');
  talker.debug('Headers: {"Authorization": "Bearer ***"}');

  Future.delayed(const Duration(milliseconds: 500), () {
    talker.info('HTTP Response: 200 OK (234ms)');
    talker.debug('Response body: {"users": [...]}');
  });
}

/// Simulates BLoC state transition logging with Talker.
void talkerBlocSimulation() {
  talker.debug('BLoC Event: LoginButtonPressed');
  talker.info('BLoC Transition: LoginInitial -> LoginLoading');

  Future.delayed(const Duration(milliseconds: 300), () {
    talker.info('BLoC Transition: LoginLoading -> LoginSuccess');
    talker.info('User authenticated successfully');
  });
}

/// Logs multiple messages to test performance.
void talkerSpamLogs(int count) {
  for (int i = 0; i < count; i++) {
    switch (i % 4) {
      case 0:
        talker.verbose('Verbose log #$i');
      case 1:
        talker.debug('Debug log #$i');
      case 2:
        talker.info('Info log #$i');
      case 3:
        talker.warning('Warning log #$i');
    }
  }
}
