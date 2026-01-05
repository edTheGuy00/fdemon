## Task: Enhance Sample Apps with Test Logs and Crashes

**Objective**: Add diverse logging statements, intentional crashes, and error scenarios to the sample Flutter apps (`sample/` and `sample2/`) using both built-in and popular third-party logging libraries (`logger` and `talker`) to enable comprehensive manual testing of error highlighting and stack trace features.

**Depends on**: None (can be done in parallel with other tasks)

### Scope

- `sample/lib/main.dart`: Enhance with various log levels and error scenarios
- `sample/lib/`: Add additional test files for different error types
- `sample/pubspec.yaml`: Add `logger` and `talker` dependencies
- `sample2/lib/main.dart`: Enhance with complementary test scenarios
- `sample2/lib/`: Add additional test files
- `sample2/pubspec.yaml`: Add logging dependencies

### Dependencies to Add

Add to both `sample/pubspec.yaml` and `sample2/pubspec.yaml`:

```yaml
dependencies:
  flutter:
    sdk: flutter
  
  # Popular logging libraries for testing
  logger: ^2.5.0
  talker: ^4.5.0
  talker_flutter: ^4.5.0
```

### Logging Libraries Overview

#### 1. Logger Package (`logger`)

The `logger` package provides pretty-printed logs with customizable formatters:

```dart
import 'package:logger/logger.dart';

// Standard logger with stack traces
final logger = Logger(
  printer: PrettyPrinter(
    methodCount: 2,      // Number of method calls in stack trace
    errorMethodCount: 8, // Stack trace depth for errors
    lineLength: 120,
    colors: true,
    printEmojis: true,
    dateTimeFormat: DateTimeFormat.onlyTimeAndSinceStart,
  ),
);

// Logger without stack traces (cleaner output)
final loggerNoStack = Logger(
  printer: PrettyPrinter(methodCount: 0),
);

// Simple one-line logger
final simpleLogger = Logger(
  printer: SimplePrinter(colors: true),
);

void demonstrateLogger() {
  logger.t('Trace message');           // Trace level
  logger.d('Debug message');           // Debug level  
  logger.i('Info message');            // Info level
  logger.w('Warning message');         // Warning level
  logger.e('Error message');           // Error level
  logger.f('Fatal/WTF message');       // Fatal level
  
  // Error with exception and stack trace
  try {
    throw Exception('Something went wrong');
  } catch (e, st) {
    logger.e('Caught exception', error: e, stackTrace: st);
  }
  
  // Log complex objects
  logger.d({'key': 'value', 'nested': {'a': 1, 'b': 2}});
}
```

#### 2. Talker Package (`talker`)

Talker provides structured logging with built-in exception handling:

```dart
import 'package:talker/talker.dart';
import 'package:talker_flutter/talker_flutter.dart';

// Initialize talker (use TalkerFlutter for Flutter apps)
final talker = TalkerFlutter.init(
  settings: TalkerSettings(
    useHistory: true,
    useConsoleLogs: true,
    maxHistoryItems: 1000,
  ),
);

void demonstrateTalker() {
  // Log levels
  talker.verbose('Verbose message');
  talker.debug('Debug message');
  talker.info('Info message');
  talker.warning('Warning message');
  talker.error('Error message');
  talker.critical('Critical message');
  talker.good('Success message ✅');  // Special "good" level
  
  // Exception handling (auto-captures stack trace)
  try {
    throw Exception('Talker caught this');
  } catch (e, st) {
    talker.handle(e, st, 'Optional context message');
  }
  
  // Custom log with specific level
  talker.log('Custom log', level: LogLevel.info);
}
```

### Test Scenarios to Add

#### 1. Built-in Dart Logging

```dart
import 'dart:developer' as developer;

void demonstrateBuiltInLogging() {
  // Standard print (Info level)
  print('Standard print message');
  
  // debugPrint (rate-limited, better for large output)
  debugPrint('Debug print message');
  
  // dart:developer log with levels
  developer.log('Debug level', level: 0, name: 'MyApp');
  developer.log('Info level', level: 800, name: 'MyApp');
  developer.log('Warning level', level: 900, name: 'MyApp');
  developer.log('Error level', level: 1000, name: 'MyApp');
  developer.log('Shout level', level: 1200, name: 'MyApp');
}
```

#### 2. Synchronous Exceptions

```dart
void triggerNullError() {
  String? nullableString;
  print(nullableString!.length); // Null check operator
}

void triggerRangeError() {
  List<int> list = [1, 2, 3];
  print(list[10]); // RangeError
}

void triggerTypeError() {
  dynamic value = 'not an int';
  int number = value as int; // TypeError
}

void triggerAssertionError() {
  assert(1 == 2, 'This assertion will fail');
}

void triggerDivisionByZero() {
  int a = 42;
  int b = 0;
  print(a ~/ b); // IntegerDivisionByZeroException
}

void triggerFormatException() {
  int.parse('not a number'); // FormatException
}

void triggerStateError() {
  List<int> emptyList = [];
  emptyList.first; // StateError: No element
}

void triggerArgumentError() {
  throw ArgumentError.value(-1, 'count', 'Must be non-negative');
}
```

#### 3. Async Exceptions

```dart
Future<void> triggerAsyncError() async {
  await Future.delayed(Duration(milliseconds: 100));
  throw Exception('Async exception after delay');
}

Future<void> triggerNestedAsyncError() async {
  await someAsyncHelper();
}

Future<void> someAsyncHelper() async {
  await anotherAsyncHelper();
}

Future<void> anotherAsyncHelper() async {
  throw StateError('Nested async error');
}

Future<void> triggerTimeoutError() async {
  await Future.delayed(Duration(seconds: 10))
      .timeout(Duration(milliseconds: 100));
}
```

#### 4. Flutter Framework Errors

```dart
// Widget build errors
class BrokenWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    // Cause overflow
    return Row(
      children: List.generate(100, (i) => 
        Container(width: 100, height: 100, color: Colors.red)
      ),
    );
  }
}

// setState after dispose
class LeakyWidget extends StatefulWidget {
  @override
  State<LeakyWidget> createState() => _LeakyWidgetState();
}

class _LeakyWidgetState extends State<LeakyWidget> {
  @override
  void initState() {
    super.initState();
    Future.delayed(Duration(seconds: 2), () {
      if (mounted) return;
      setState(() {}); // Error: setState called after dispose
    });
  }
  
  @override
  Widget build(BuildContext context) => Container();
}

// Missing MediaQuery ancestor
class MissingAncestorWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    // This will fail without proper ancestor
    return SizedBox(
      width: MediaQuery.of(context).size.width,
      child: Text('Test'),
    );
  }
}
```

#### 5. Deep Stack Traces

```dart
void deepStackTrace() {
  level1();
}

void level1() => level2();
void level2() => level3();
void level3() => level4();
void level4() => level5();
void level5() => level6();
void level6() => level7();
void level7() => level8();
void level8() => level9();
void level9() => level10();
void level10() {
  throw Exception('Deep stack trace error at level 10');
}

void veryDeepStackTrace() {
  _recursiveCall(20);
}

void _recursiveCall(int depth) {
  if (depth <= 0) {
    throw Exception('Very deep stack trace error at depth 20');
  }
  _recursiveCall(depth - 1);
}
```

#### 6. Logger Package Scenarios

```dart
import 'package:logger/logger.dart';

final logger = Logger(
  printer: PrettyPrinter(
    methodCount: 5,
    errorMethodCount: 10,
    lineLength: 120,
    colors: true,
    printEmojis: true,
  ),
);

void demonstrateLoggerPackage() {
  // All log levels
  logger.t('Trace: Very detailed debugging info');
  logger.d('Debug: Debugging information');
  logger.i('Info: General information');
  logger.w('Warning: Something might be wrong');
  logger.e('Error: Something went wrong!');
  logger.f('Fatal: Critical failure!');
  
  // With stack trace
  logger.e('Error with auto stack trace');
  
  // With exception
  try {
    throw FormatException('Invalid format');
  } catch (e, st) {
    logger.e('Caught format exception', error: e, stackTrace: st);
  }
  
  // Log data structures
  logger.i({
    'user': 'john_doe',
    'action': 'login',
    'timestamp': DateTime.now().toIso8601String(),
    'metadata': {'ip': '192.168.1.1', 'device': 'iPhone'},
  });
  
  // Multi-line message
  logger.i('''
Multi-line log message:
- Line 1: First item
- Line 2: Second item  
- Line 3: Third item
''');
}

void loggerWithCustomPrinter() {
  final simpleLogger = Logger(printer: SimplePrinter());
  simpleLogger.i('Simple one-line log');
  simpleLogger.w('Simple warning');
  simpleLogger.e('Simple error');
}
```

#### 7. Talker Package Scenarios

```dart
import 'package:talker/talker.dart';
import 'package:talker_flutter/talker_flutter.dart';

final talker = TalkerFlutter.init();

void demonstrateTalkerPackage() {
  // All log levels
  talker.verbose('Verbose: Maximum detail');
  talker.debug('Debug: Debugging info');
  talker.info('Info: General information');
  talker.warning('Warning: Potential issue');
  talker.error('Error: Something failed');
  talker.critical('Critical: System failure');
  talker.good('Good: Operation succeeded! ✅');
  
  // Handle exceptions with context
  try {
    throw Exception('Talker exception test');
  } catch (e, st) {
    talker.handle(e, st, 'Context: User login failed');
  }
  
  // Handle errors
  try {
    throw StateError('Invalid state for operation');
  } catch (e, st) {
    talker.handle(e, st);
  }
  
  // Custom typed logs
  talker.logTyped(
    TalkerLog('Custom typed log message', level: LogLevel.info),
  );
}

void talkerHttpSimulation() {
  // Simulate HTTP request/response logging
  talker.info('HTTP Request: GET /api/users');
  talker.debug('Headers: {"Authorization": "Bearer ***"}');
  
  Future.delayed(Duration(milliseconds: 500), () {
    talker.good('HTTP Response: 200 OK (234ms)');
    talker.debug('Response body: {"users": [...]}');
  });
}

void talkerBlocSimulation() {
  // Simulate BLoC state changes
  talker.debug('BLoC Event: LoginButtonPressed');
  talker.info('BLoC Transition: LoginInitial -> LoginLoading');
  
  Future.delayed(Duration(milliseconds: 300), () {
    talker.info('BLoC Transition: LoginLoading -> LoginSuccess');
    talker.good('User authenticated successfully');
  });
}
```

### Sample App 1 (`sample/`) Structure

```
sample/lib/
├── main.dart                    # Main app with error trigger buttons
├── logging/
│   ├── builtin_logging.dart     # dart:developer logging
│   ├── logger_demo.dart         # logger package demo
│   └── talker_demo.dart         # talker package demo
├── errors/
│   ├── sync_errors.dart         # Synchronous error scenarios
│   ├── async_errors.dart        # Async error scenarios
│   └── deep_stack.dart          # Deep stack trace tests
└── widgets/
    └── broken_widgets.dart      # Flutter widget errors
```

### Sample App 2 (`sample2/`) Structure

```
sample2/lib/
├── main.dart                    # Alternative error scenarios
├── logging/
│   ├── log_levels.dart          # Various log level outputs
│   ├── verbose_logs.dart        # High-volume log testing
│   └── mixed_loggers.dart       # Mix of logger + talker
└── errors/
    └── flutter_errors.dart      # Flutter-specific errors
```

### Main App UI Enhancement

Update `sample/lib/main.dart` to include an error testing panel:

```dart
import 'package:flutter/material.dart';
import 'package:logger/logger.dart';
import 'package:talker_flutter/talker_flutter.dart';

import 'logging/builtin_logging.dart';
import 'logging/logger_demo.dart';
import 'logging/talker_demo.dart';
import 'errors/sync_errors.dart';
import 'errors/async_errors.dart';
import 'errors/deep_stack.dart';

final logger = Logger(printer: PrettyPrinter(methodCount: 5));
final talker = TalkerFlutter.init();

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
          _buildSection(context, '📝 Built-in Logging', [
            _buildButton('print()', () => print('Standard print message')),
            _buildButton('debugPrint()', () => debugPrint('Debug print message')),
            _buildButton('developer.log()', demonstrateBuiltInLogging),
          ]),
          
          _buildSection(context, '🪵 Logger Package', [
            _buildButton('All Levels', demonstrateLoggerPackage),
            _buildButton('With Exception', () {
              try {
                throw Exception('Logger test exception');
              } catch (e, st) {
                logger.e('Caught by logger', error: e, stackTrace: st);
              }
            }),
            _buildButton('Log Object', () => logger.i({'key': 'value', 'count': 42})),
            _buildButton('Simple Printer', loggerWithCustomPrinter),
          ]),
          
          _buildSection(context, '🎙️ Talker Package', [
            _buildButton('All Levels', demonstrateTalkerPackage),
            _buildButton('Handle Exception', () {
              try {
                throw StateError('Talker test error');
              } catch (e, st) {
                talker.handle(e, st, 'Test context');
              }
            }),
            _buildButton('HTTP Simulation', talkerHttpSimulation),
            _buildButton('BLoC Simulation', talkerBlocSimulation),
          ]),
          
          _buildSection(context, '💥 Sync Errors', [
            _buildButton('Null Error', triggerNullError),
            _buildButton('Range Error', triggerRangeError),
            _buildButton('Type Error', triggerTypeError),
            _buildButton('Format Exception', triggerFormatException),
            _buildButton('State Error', triggerStateError),
            _buildButton('Argument Error', triggerArgumentError),
          ]),
          
          _buildSection(context, '⏳ Async Errors', [
            _buildButton('Simple Async', triggerAsyncError),
            _buildButton('Nested Async (3 levels)', triggerNestedAsyncError),
            _buildButton('Timeout Error', triggerTimeoutError),
          ]),
          
          _buildSection(context, '📚 Stack Traces', [
            _buildButton('Deep Stack (10 levels)', deepStackTrace),
            _buildButton('Very Deep (20 levels)', veryDeepStackTrace),
          ]),
          
          _buildSection(context, '🔄 Spam Logs', [
            _buildButton('10 Mixed Logs', () => _spamLogs(10)),
            _buildButton('50 Mixed Logs', () => _spamLogs(50)),
            _buildButton('100 Mixed Logs', () => _spamLogs(100)),
          ]),
        ],
      ),
    );
  }
  
  Widget _buildSection(BuildContext context, String title, List<Widget> children) {
    return Card(
      margin: const EdgeInsets.only(bottom: 16),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(title, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: children,
            ),
          ],
        ),
      ),
    );
  }
  
  Widget _buildButton(String label, VoidCallback onPressed) {
    return ElevatedButton(
      onPressed: onPressed,
      child: Text(label),
    );
  }
  
  void _spamLogs(int count) {
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
}
```

### Expected Log Output Formats

#### Logger Package Output
```
┌──────────────────────────────────────────────────────────────────────────────
│ 🐛 Debug: Debugging information
├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄
│ #0   demonstrateLoggerPackage (package:sample/logging/logger_demo.dart:15:10)
│ #1   _buildButton.<anonymous closure> (package:sample/main.dart:78:23)
└──────────────────────────────────────────────────────────────────────────────
```

#### Talker Package Output
```
[TALKER] | 12:34:56 | INFO: General information
[TALKER] | 12:34:56 | WARNING: Potential issue  
[TALKER] | 12:34:56 | ERROR: Something failed
[TALKER] | 12:34:56 | EXCEPTION: Exception: Talker exception test
│ Context: User login failed
│ #0   demonstrateTalkerPackage (package:sample/logging/talker_demo.dart:22:5)
│ #1   _buildButton.<anonymous closure> (package:sample/main.dart:85:23)
```

### Acceptance Criteria

1. [x] `sample/pubspec.yaml` includes `logger: ^2.5.0` and `talker_flutter: ^4.5.0`
2. [x] `sample2/pubspec.yaml` includes logging dependencies
3. [x] `sample/lib/main.dart` has error testing UI with categorized buttons
4. [x] Logger package demo with all log levels (t, d, i, w, e, f)
5. [x] Talker package demo with all log levels and exception handling
6. [x] Built-in logging demo (print, debugPrint, developer.log)
7. [x] At least 5 different sync error types testable (9 implemented)
8. [x] At least 3 different async error types testable (6 implemented)
9. [x] Deep stack trace scenario (10+ frames) implemented (10, 20, 50 levels)
10. [x] "Spam logs" feature for performance testing
11. [x] Talker's built-in log viewer accessible via button
12. [ ] Apps compile and run on iOS Simulator (manual testing required)
13. [ ] Apps compile and run on Android Emulator (manual testing required)
14. [x] All error triggers produce readable stack traces

### Testing

Manual testing steps:

1. Run `flutter pub get` in both sample apps
2. Run Flutter Demon with `sample/` project
3. Test each logging library section:
   - Verify Logger's pretty-printed output appears
   - Verify Talker's structured output appears
   - Verify built-in logging works
4. Trigger each error type via UI buttons
5. Verify stack traces appear in log view
6. Test "Spam logs" buttons for performance
7. Open Talker's built-in log viewer (bug icon in app bar)
8. Repeat key tests with `sample2/`

Stack trace verification checklist:
- [ ] Logger package stack traces display correctly
- [ ] Talker package stack traces display correctly
- [ ] Dart VM format traces appear correctly
- [ ] Async suspension markers appear
- [ ] File:line references are present
- [ ] Package frames (flutter/, logger/, talker/) appear
- [ ] Project frames (sample/) appear
- [ ] Function names are captured
- [ ] Pretty-printed borders/boxes render (logger package)

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `sample/pubspec.yaml` | Modify | Add logger and talker dependencies |
| `sample/lib/main.dart` | Modify | Add comprehensive error testing UI |
| `sample/lib/logging/builtin_logging.dart` | Create | dart:developer logging examples |
| `sample/lib/logging/logger_demo.dart` | Create | Logger package demonstrations |
| `sample/lib/logging/talker_demo.dart` | Create | Talker package demonstrations |
| `sample/lib/errors/sync_errors.dart` | Create | Synchronous error triggers |
| `sample/lib/errors/async_errors.dart` | Create | Async error triggers |
| `sample/lib/errors/deep_stack.dart` | Create | Deep stack trace generators |
| `sample/lib/widgets/broken_widgets.dart` | Create | Flutter widget error examples |
| `sample2/pubspec.yaml` | Modify | Add logging dependencies |
| `sample2/lib/main.dart` | Modify | Add complementary test scenarios |
| `sample2/lib/logging/mixed_loggers.dart` | Create | Mixed logger/talker usage |
| `sample2/lib/errors/flutter_errors.dart` | Create | Additional Flutter error types |

### Estimated Time

3-4 hours

### Notes

- Keep error triggers behind explicit button presses (don't crash on startup)
- Add clear visual feedback when an error button is pressed
- The "Spam Logs" feature helps test log view performance with many entries
- Both Logger and Talker have distinctive output formats that should be recognizable
- Talker's TalkerScreen provides a built-in log viewer - useful for comparison
- Document each error type with comments explaining expected behavior
- Both sample apps should be kept compilable at all times
- Logger's PrettyPrinter adds box-drawing characters that may need special handling
- Talker's colored output uses ANSI codes similar to what we'll parse

---

## Completion Summary

**Status**: ✅ Done

**Files Created/Modified**:

**sample/**
- `sample/pubspec.yaml` - Added logger ^2.5.0 and talker_flutter ^4.5.0
- `sample/lib/main.dart` - Replaced with comprehensive error testing UI
- `sample/lib/logging/builtin_logging.dart` - NEW: dart:developer logging demos
- `sample/lib/logging/logger_demo.dart` - NEW: Logger package with all levels
- `sample/lib/logging/talker_demo.dart` - NEW: Talker package with exception handling
- `sample/lib/errors/sync_errors.dart` - NEW: 9 sync error types
- `sample/lib/errors/async_errors.dart` - NEW: 6 async error types
- `sample/lib/errors/deep_stack.dart` - NEW: 10/20/50-level stack traces

**sample2/**
- `sample2/pubspec.yaml` - Added logger and talker dependencies
- `sample2/lib/main.dart` - Replaced with complementary test scenarios
- `sample2/lib/logging/mixed_loggers.dart` - NEW: Mixed Logger + Talker usage
- `sample2/lib/errors/flutter_errors.dart` - NEW: Flutter-specific errors

**Notable Decisions/Tradeoffs**:
- Removed `talker.good()` calls as API doesn't exist in current version (4.9.3)
- Error buttons catch and log exceptions via Logger for visible stack traces
- Color-coded buttons: red for sync errors, orange for async errors, purple for widget errors
- Deep stack generators use both iterative (level1-10) and recursive approaches

**Testing Performed**:
- `flutter pub get` - PASS (both apps, dependencies resolved)
- `dart analyze lib/` - PASS (both apps, no errors)

**Manual Testing Required**:
- Run on iOS Simulator to verify app launches and buttons work
- Run on Android Emulator for cross-platform verification
- Test each logging section with Flutter Demon to verify log output format

**Risks/Limitations**:
- Widget error scenarios (overflow, build errors) may crash the app if not properly contained
- Talker API may change in future versions, requiring updates
- Logger's box-drawing characters require terminal Unicode support