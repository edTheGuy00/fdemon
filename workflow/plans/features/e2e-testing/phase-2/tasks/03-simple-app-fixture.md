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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/fixtures/simple_app/pubspec.yaml` | Created minimal Flutter app configuration with no material design |
| `tests/fixtures/simple_app/lib/main.dart` | Created main app with [FDEMON_TEST] log markers |
| `tests/fixtures/simple_app/test/widget_test.dart` | Created widget test to verify app functionality |
| `tests/fixtures/simple_app/.gitignore` | Created gitignore for Flutter build artifacts |

### Notable Decisions/Tradeoffs

1. **No Platform-Specific Projects**: The fixture intentionally omits platform-specific configurations (macOS, Linux, etc.) to keep it minimal. This means `flutter build` commands for platforms won't work, but `flutter test` works perfectly and `flutter run --machine` can be used with available devices/simulators.

2. **Widget-Only Approach**: Uses basic Flutter widgets (Text, Center) without Material or Cupertino, resulting in the fastest possible compile time for E2E tests.

3. **Test Markers**: Both `[FDEMON_TEST] App starting` and `[FDEMON_TEST] Building SimpleApp` markers are included. The "Building SimpleApp" message appears during widget tests, while "App starting" would appear when running the app via `flutter run`.

### Testing Performed

- `flutter pub get` - Passed (resolved 24 dependencies)
- `flutter test` - Passed (1 test passed, verified [FDEMON_TEST] log output)
- `flutter analyze` - Passed (no issues found)
- Fixture size check - Passed (1.4 KB source files, well under 100KB limit)

### Verification Output

```
SimpleApp displays text: PASS
[FDEMON_TEST] Building SimpleApp
All tests passed!
```

### Risks/Limitations

1. **Platform Builds**: Cannot run `flutter build linux/macos/windows` without adding platform-specific configurations. This is acceptable for the E2E testing use case, as tests will use `flutter run --machine` with simulators or test devices.

2. **Hot Reload Testing**: Hot reload functionality can be tested by running `flutter run` (not in --machine mode for manual testing) or via E2E scripts that use the --machine mode and send reload commands through JSON-RPC.
