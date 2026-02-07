// ignore_for_file: dangling_library_doc_comments, avoid_print

/// Flutter Demon Test App - Sample 1
///
/// This app provides a comprehensive test suite for Flutter Demon's
/// log viewing, error highlighting, and stack trace display features.

import 'package:flutter/material.dart';
import 'package:talker_flutter/talker_flutter.dart';

import 'logging/builtin_logging.dart';
import 'logging/logger_demo.dart';
import 'logging/talker_demo.dart';
import 'errors/sync_errors.dart';
import 'errors/async_errors.dart';
import 'errors/deep_stack.dart';

void main() {
  runApp(const TestLoggingApp());
}

class TestLoggingApp extends StatelessWidget {
  const TestLoggingApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Flutter Demon Test App',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.deepPurple),
        useMaterial3: true,
      ),
      home: const ErrorTestingPage(),
    );
  }
}

class ErrorTestingPage extends StatelessWidget {
  const ErrorTestingPage({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Flutter Demon Test Suite'),
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
        actions: [
          IconButton(
            icon: const Icon(Icons.bug_report),
            onPressed: () => Navigator.of(context).push(
              MaterialPageRoute(builder: (_) => TalkerScreen(talker: talker)),
            ),
            tooltip: 'Open Talker Logs',
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          _buildSection(context, 'Built-in Logging', [
            _buildButton('print()', () => print('Standard print message')),
            _buildButton(
              'debugPrint()',
              () => debugPrint('Debug print message'),
            ),
            _buildButton('developer.log()', demonstrateBuiltInLogging),
            _buildButton('Log + Metadata', demonstrateLogWithMetadata),
            _buildButton('Log + Error', demonstrateLogWithError),
          ]),
          _buildSection(context, 'Logger Package', [
            _buildButton('All Levels', demonstrateLoggerPackage),
            _buildButton('With Exception', demonstrateLoggerWithException),
            _buildButton('Log Object', demonstrateLoggerWithData),
            _buildButton('Multi-line', demonstrateLoggerMultiLine),
            _buildButton('Simple Printer', loggerWithCustomPrinter),
            _buildButton('No Stack Trace', loggerWithoutStackTrace),
          ]),
          _buildSection(context, 'Talker Package', [
            _buildButton('All Levels', demonstrateTalkerPackage),
            _buildButton('Handle Exception', demonstrateTalkerWithException),
            _buildButton('Handle Error', demonstrateTalkerWithError),
            _buildButton('Typed Log', demonstrateTalkerTypedLog),
            _buildButton('HTTP Sim', talkerHttpSimulation),
            _buildButton('BLoC Sim', talkerBlocSimulation),
          ]),
          _buildSection(context, 'Sync Errors', [
            _buildErrorButton('Null Error', triggerNullError),
            _buildErrorButton('Range Error', triggerRangeError),
            _buildErrorButton('Type Error', triggerTypeError),
            _buildErrorButton('Format Exception', triggerFormatException),
            _buildErrorButton('State Error', triggerStateError),
            _buildErrorButton('Argument Error', triggerArgumentError),
            _buildErrorButton('Unsupported Error', triggerUnsupportedError),
            _buildErrorButton('Custom Exception', triggerCustomException),
            _buildErrorButton('Long Message', triggerLongErrorMessage),
          ]),
          _buildSection(context, 'Async Errors', [
            _buildAsyncErrorButton('Simple Async', triggerAsyncError),
            _buildAsyncErrorButton(
              'Nested (3 levels)',
              triggerNestedAsyncError,
            ),
            _buildAsyncErrorButton('Timeout Error', triggerTimeoutError),
            _buildAsyncErrorButton(
              'Multi Suspensions',
              triggerMultipleAsyncSuspensions,
            ),
            _buildButton('Uncaught Async', triggerUncaughtAsyncError),
            _buildAsyncErrorButton('Stream Error', triggerStreamError),
          ]),
          _buildSection(context, 'Stack Traces', [
            _buildErrorButton('Deep (10 levels)', deepStackTrace),
            _buildErrorButton('Very Deep (20)', veryDeepStackTrace),
            _buildErrorButton('Extreme (50)', extremelyDeepStackTrace),
            _buildErrorButton('Mixed Closures', mixedStackTrace),
            _buildAsyncErrorButton('Async Deep', asyncDeepStackTrace),
          ]),
          _buildSection(context, 'Spam Logs', [
            _buildButton('10 Mixed', () => _spamMixedLogs(10)),
            _buildButton('50 Mixed', () => _spamMixedLogs(50)),
            _buildButton('100 Mixed', () => _spamMixedLogs(100)),
            _buildButton('10 Logger', () => _spamLoggerLogs(10)),
            _buildButton('10 Talker', () => talkerSpamLogs(10)),
            _buildButton('10 Rapid', () => demonstrateRapidLogging(10)),
          ]),
        ],
      ),
    );
  }

  Widget _buildSection(
    BuildContext context,
    String title,
    List<Widget> children,
  ) {
    return Card(
      margin: const EdgeInsets.only(bottom: 16),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(title, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 12),
            Wrap(spacing: 8, runSpacing: 8, children: children),
          ],
        ),
      ),
    );
  }

  Widget _buildButton(String label, VoidCallback onPressed) {
    return ElevatedButton(onPressed: onPressed, child: Text(label));
  }

  Widget _buildErrorButton(String label, VoidCallback errorFunction) {
    return ElevatedButton(
      style: ElevatedButton.styleFrom(
        backgroundColor: Colors.red.shade100,
        foregroundColor: Colors.red.shade900,
      ),
      onPressed: () {
        try {
          errorFunction();
        } catch (e, st) {
          // Log the error so it appears in Flutter Demon
          logger.e('Error triggered: $label', error: e, stackTrace: st);
        }
      },
      child: Text(label),
    );
  }

  Widget _buildAsyncErrorButton(
    String label,
    Future<void> Function() errorFunction,
  ) {
    return ElevatedButton(
      style: ElevatedButton.styleFrom(
        backgroundColor: Colors.orange.shade100,
        foregroundColor: Colors.orange.shade900,
      ),
      onPressed: () async {
        try {
          await errorFunction();
        } catch (e, st) {
          logger.e('Async error triggered: $label', error: e, stackTrace: st);
        }
      },
      child: Text(label),
    );
  }

  void _spamMixedLogs(int count) {
    for (int i = 0; i < count; i++) {
      switch (i % 5) {
        case 0:
          logger.t('Trace log #$i');
        case 1:
          logger.i('Info log #$i');
        case 2:
          talker.warning('Warning log #$i');
        case 3:
          print('Print log #$i');
        case 4:
          talker.debug('Debug log #$i');
      }
    }
  }

  void _spamLoggerLogs(int count) {
    for (int i = 0; i < count; i++) {
      switch (i % 6) {
        case 0:
          logger.t('Trace #$i');
        case 1:
          logger.d('Debug #$i');
        case 2:
          logger.i('Info #$i');
        case 3:
          logger.w('Warning #$i');
        case 4:
          logger.e('Error #$i');
        case 5:
          logger.f('Fatal #$i');
      }
    }
  }
}
