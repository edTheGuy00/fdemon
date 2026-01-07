## Task: Create error_app Flutter Fixture

**Objective**: Create a Flutter application with intentional errors that can be used to test fdemon's error handling, recovery, and display capabilities.

**Depends on**: 03-simple-app-fixture (can use as starting point)

### Scope

- `tests/fixtures/error_app/`: **NEW** - Flutter app with intentional errors

### Details

Create a Flutter app that can trigger various error conditions:
1. Compile-time errors (syntax, type errors)
2. Runtime errors (null reference, assertion failures)
3. Widget build errors (RenderFlex overflow, etc.)
4. Multiple error files for targeted testing

#### Directory Structure

```
tests/fixtures/error_app/
├── pubspec.yaml
├── lib/
│   ├── main.dart              # Working entry point
│   ├── main_syntax_error.dart # Syntax error variant
│   ├── main_type_error.dart   # Type error variant
│   ├── main_runtime_error.dart # Runtime error variant
│   └── main_widget_error.dart  # Widget error variant
├── test/
│   └── widget_test.dart
└── .gitignore
```

#### lib/main.dart (Working version)

```dart
import 'package:flutter/widgets.dart';

void main() {
  debugPrint('[FDEMON_TEST] error_app starting (working mode)');
  runApp(const ErrorApp());
}

class ErrorApp extends StatelessWidget {
  const ErrorApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: Text(
        'Error App - Working Mode',
        textDirection: TextDirection.ltr,
      ),
    );
  }
}
```

#### lib/main_syntax_error.dart

```dart
import 'package:flutter/widgets.dart';

void main() {
  // Missing closing brace - syntax error
  runApp(const ErrorApp()
}

class ErrorApp extends StatelessWidget {
  const ErrorApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: Text('Syntax Error App', textDirection: TextDirection.ltr),
    );
  }
}
```

#### lib/main_type_error.dart

```dart
import 'package:flutter/widgets.dart';

void main() {
  // Type error: assigning String to int
  int count = 'not a number';
  debugPrint('Count: $count');
  runApp(const ErrorApp());
}

class ErrorApp extends StatelessWidget {
  const ErrorApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: Text('Type Error App', textDirection: TextDirection.ltr),
    );
  }
}
```

#### lib/main_runtime_error.dart

```dart
import 'package:flutter/widgets.dart';

void main() {
  debugPrint('[FDEMON_TEST] error_app starting (runtime error mode)');
  runApp(const RuntimeErrorApp());
}

class RuntimeErrorApp extends StatelessWidget {
  const RuntimeErrorApp({super.key});

  @override
  Widget build(BuildContext context) {
    // Trigger null dereference
    String? nullValue;
    return Center(
      child: Text(
        nullValue!.toUpperCase(),  // Will throw
        textDirection: TextDirection.ltr,
      ),
    );
  }
}
```

#### lib/main_widget_error.dart

```dart
import 'package:flutter/widgets.dart';

void main() {
  debugPrint('[FDEMON_TEST] error_app starting (widget error mode)');
  runApp(const WidgetErrorApp());
}

class WidgetErrorApp extends StatelessWidget {
  const WidgetErrorApp({super.key});

  @override
  Widget build(BuildContext context) {
    // Throw during build
    throw FlutterError('Intentional widget build error for testing');
  }
}
```

#### Test Script Usage

```bash
# Test compile error handling
cp tests/fixtures/error_app/lib/main_syntax_error.dart \
   tests/fixtures/error_app/lib/main.dart
flutter run --machine  # Should show compile error

# Test runtime error handling
cp tests/fixtures/error_app/lib/main_runtime_error.dart \
   tests/fixtures/error_app/lib/main.dart
flutter run --machine  # Should show runtime error

# Restore working version
git checkout tests/fixtures/error_app/lib/main.dart
```

### Acceptance Criteria

1. Default `main.dart` runs without errors
2. `main_syntax_error.dart` fails with clear syntax error message
3. `main_type_error.dart` fails with clear type error message
4. `main_runtime_error.dart` compiles but throws at runtime
5. `main_widget_error.dart` compiles but throws during widget build
6. Error messages include file and line information
7. Scripts can swap main.dart variants for targeted testing

### Testing

```bash
cd tests/fixtures/error_app

# Verify working version runs
flutter run --machine

# Test each error variant
for variant in syntax_error type_error; do
  cp lib/main_${variant}.dart lib/main.dart.bak
  cp lib/main_${variant}.dart lib/main.dart
  flutter analyze lib/main.dart  # Should show errors
  cp lib/main.dart.bak lib/main.dart
done

# Test runtime errors (need to actually run)
cp lib/main_runtime_error.dart lib/main.dart
flutter run --machine  # Observe error output
```

### Notes

- Keep the working main.dart as the default to not break other tests
- Error variants should have descriptive names
- Consider adding a script to swap variants automatically
- Runtime errors are harder to test - may need timeout handling
- Widget errors produce Flutter's red error screen

---

## Completion Summary

**Status:** Not Started
