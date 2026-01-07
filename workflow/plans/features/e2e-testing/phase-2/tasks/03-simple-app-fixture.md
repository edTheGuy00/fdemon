## Task: Create simple_app Flutter Fixture

**Objective**: Create a minimal, runnable Flutter application that can be used for basic E2E testing of startup, hot reload, and shutdown workflows.

**Depends on**: None (can be done in parallel with Docker setup)

### Scope

- `tests/fixtures/simple_app/`: **NEW** - Minimal Flutter application

### Details

Create the simplest possible Flutter app that:
1. Compiles and runs successfully
2. Has a modifiable widget for hot reload testing
3. Logs a startup message for verification
4. Keeps dependencies minimal for fast compilation

#### Directory Structure

```
tests/fixtures/simple_app/
├── pubspec.yaml
├── lib/
│   └── main.dart
├── test/
│   └── widget_test.dart
└── .gitignore
```

#### pubspec.yaml

```yaml
name: simple_app
description: Minimal Flutter app for E2E testing
publish_to: 'none'

version: 1.0.0+1

environment:
  sdk: '>=3.0.0 <4.0.0'

dependencies:
  flutter:
    sdk: flutter

dev_dependencies:
  flutter_test:
    sdk: flutter

flutter:
  uses-material-design: false  # Minimize dependencies
```

#### lib/main.dart

```dart
import 'package:flutter/widgets.dart';

void main() {
  // Log message that E2E tests can verify
  debugPrint('[FDEMON_TEST] App starting');
  runApp(const SimpleApp());
}

class SimpleApp extends StatelessWidget {
  const SimpleApp({super.key});

  @override
  Widget build(BuildContext context) {
    debugPrint('[FDEMON_TEST] Building SimpleApp');
    return const Center(
      child: Text(
        'Hello from simple_app',
        textDirection: TextDirection.ltr,
        style: TextStyle(fontSize: 24),
      ),
    );
  }
}
```

#### Key Considerations

1. **Minimal Dependencies**:
   - No Material or Cupertino (uses basic widgets)
   - No external packages
   - Fastest possible compile time

2. **Test Markers**:
   - `[FDEMON_TEST]` prefix for log messages
   - Easy to grep for in test output

3. **Hot Reload Friendly**:
   - Text content can be modified for reload verification
   - Widget structure simple enough to detect changes

4. **No Assets**:
   - No images, fonts, or other assets
   - Keeps fixture size minimal

### Acceptance Criteria

1. `flutter pub get` succeeds in the fixture directory
2. `flutter build linux --debug` succeeds (or relevant platform)
3. `flutter run --machine` starts and outputs JSON-RPC events
4. App logs `[FDEMON_TEST] App starting` on startup
5. Hot reload works when main.dart is modified
6. Total fixture size <100KB (excluding build artifacts)

### Testing

```bash
cd tests/fixtures/simple_app

# Verify dependencies
flutter pub get

# Verify builds
flutter build linux --debug

# Verify runs (Ctrl+C to stop)
flutter run --machine

# Verify hot reload
# 1. Start app: flutter run
# 2. Modify text in main.dart
# 3. Press 'r' to reload
# 4. Verify text changed
```

### Notes

- This fixture is the foundation for other fixtures (error_app, etc.)
- Keep it minimal - complexity should be added in other fixtures
- The `[FDEMON_TEST]` markers are critical for script verification
- Consider adding a `.fdemon/` directory for testing config loading

---

## Completion Summary

**Status:** Not Started
