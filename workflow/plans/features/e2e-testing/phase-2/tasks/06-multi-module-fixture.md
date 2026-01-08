## Task: Create multi_module Flutter Fixture

**Objective**: Create a Flutter monorepo structure with multiple packages to test fdemon's handling of complex project layouts with shared dependencies.

**Depends on**: 03-simple-app-fixture (reuses patterns)

### Scope

- `tests/fixtures/multi_module/`: **NEW** - Monorepo with multiple Flutter packages

### Details

Create a monorepo structure that:
1. Has multiple Flutter packages in packages/
2. Has a main app that depends on local packages
3. Tests project discovery with multiple runnable targets
4. Validates fdemon's handling of path dependencies

#### Directory Structure

```
tests/fixtures/multi_module/
├── pubspec.yaml                  # Workspace root (optional)
├── melos.yaml                    # Melos config (optional, for reference)
├── apps/
│   └── main_app/
│       ├── pubspec.yaml
│       └── lib/
│           └── main.dart
├── packages/
│   ├── core/
│   │   ├── pubspec.yaml
│   │   └── lib/
│   │       └── core.dart
│   └── ui_components/
│       ├── pubspec.yaml
│       └── lib/
│           └── ui_components.dart
└── .gitignore
```

#### apps/main_app/pubspec.yaml

```yaml
name: main_app
description: Main application for multi-module fixture
publish_to: 'none'

version: 1.0.0+1

environment:
  sdk: '>=3.0.0 <4.0.0'

dependencies:
  flutter:
    sdk: flutter
  core:
    path: ../../packages/core
  ui_components:
    path: ../../packages/ui_components

dev_dependencies:
  flutter_test:
    sdk: flutter

flutter:
  uses-material-design: false
```

#### apps/main_app/lib/main.dart

```dart
import 'package:flutter/widgets.dart';
import 'package:core/core.dart';
import 'package:ui_components/ui_components.dart';

void main() {
  debugPrint('[FDEMON_TEST] multi_module main_app starting');
  CoreLogger.log('App initialized');
  runApp(const MainApp());
}

class MainApp extends StatelessWidget {
  const MainApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: AppTitle(text: 'Multi-Module App'),
    );
  }
}
```

#### packages/core/pubspec.yaml

```yaml
name: core
description: Core utilities for multi-module fixture
publish_to: 'none'

version: 0.0.1

environment:
  sdk: '>=3.0.0 <4.0.0'

dependencies:
  flutter:
    sdk: flutter
```

#### packages/core/lib/core.dart

```dart
library core;

class CoreLogger {
  static void log(String message) {
    print('[CORE] $message');
  }
}

class AppConfig {
  static const String appName = 'Multi-Module Test';
  static const String version = '1.0.0';
}
```

#### packages/ui_components/pubspec.yaml

```yaml
name: ui_components
description: Shared UI components for multi-module fixture
publish_to: 'none'

version: 0.0.1

environment:
  sdk: '>=3.0.0 <4.0.0'

dependencies:
  flutter:
    sdk: flutter
  core:
    path: ../core
```

#### packages/ui_components/lib/ui_components.dart

```dart
library ui_components;

import 'package:flutter/widgets.dart';
import 'package:core/core.dart';

class AppTitle extends StatelessWidget {
  final String text;

  const AppTitle({super.key, required this.text});

  @override
  Widget build(BuildContext context) {
    CoreLogger.log('Rendering AppTitle: $text');
    return Text(
      text,
      textDirection: TextDirection.ltr,
      style: const TextStyle(fontSize: 24, fontWeight: FontWeight.bold),
    );
  }
}
```

#### Optional: melos.yaml (for reference)

```yaml
name: multi_module_fixture
packages:
  - apps/**
  - packages/**

scripts:
  analyze:
    exec: flutter analyze
  test:
    exec: flutter test
```

### Key Considerations

1. **Monorepo Structure**:
   - apps/ contains runnable applications
   - packages/ contains shared libraries
   - Common pattern in large Flutter projects

2. **Dependency Chain**:
   - main_app -> ui_components -> core
   - Tests transitive path dependencies

3. **Project Discovery**:
   - fdemon should find main_app as runnable
   - packages/ should not be directly runnable
   - Could support running from root with selection

4. **Melos Compatibility**:
   - Include melos.yaml for reference
   - fdemon should work alongside melos

### Acceptance Criteria

1. `flutter pub get` succeeds in apps/main_app/
2. `flutter pub get` succeeds in packages/core/
3. `flutter pub get` succeeds in packages/ui_components/
4. `flutter run --machine` in apps/main_app/ starts the app
5. App logs `[FDEMON_TEST] multi_module main_app starting`
6. Path dependencies resolve correctly
7. fdemon discovers apps/main_app/ as runnable target

### Testing

```bash
# Get dependencies for all packages
cd tests/fixtures/multi_module/packages/core && flutter pub get
cd tests/fixtures/multi_module/packages/ui_components && flutter pub get
cd tests/fixtures/multi_module/apps/main_app && flutter pub get

# Run main app
cd tests/fixtures/multi_module/apps/main_app
flutter run --machine

# Test fdemon discovery (once implemented)
cd tests/fixtures/multi_module
fdemon  # Should discover apps/main_app
```

### Notes

- Monorepo patterns are increasingly common (melos, very_good_cli)
- This fixture tests a realistic large-project structure
- Consider adding more apps (e.g., admin_app) for multiple targets
- Path resolution is critical - ensure relative paths work in Docker

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/fixtures/multi_module/.gitignore` | Created Flutter build artifacts and IDE ignore patterns |
| `tests/fixtures/multi_module/melos.yaml` | Created Melos workspace configuration for reference |
| `tests/fixtures/multi_module/packages/core/pubspec.yaml` | Created core package metadata with Flutter SDK dependency |
| `tests/fixtures/multi_module/packages/core/lib/core.dart` | Created CoreLogger and AppConfig classes |
| `tests/fixtures/multi_module/packages/ui_components/pubspec.yaml` | Created UI components package with path dependency on core |
| `tests/fixtures/multi_module/packages/ui_components/lib/ui_components.dart` | Created AppTitle widget that uses CoreLogger |
| `tests/fixtures/multi_module/apps/main_app/pubspec.yaml` | Created main app with path dependencies on core and ui_components |
| `tests/fixtures/multi_module/apps/main_app/lib/main.dart` | Created main app entry point with [FDEMON_TEST] log marker |

### Notable Decisions/Tradeoffs

1. **Dependency Chain**: Implemented main_app -> ui_components -> core transitive dependency chain to validate fdemon's path dependency resolution
2. **Minimal Design**: Used minimal Flutter widgets (no Material) to reduce dependency footprint and startup time, consistent with simple_app fixture pattern
3. **Test Marker**: Included `[FDEMON_TEST] multi_module main_app starting` log message for automated test verification
4. **Melos Configuration**: Included melos.yaml for reference even though melos is optional, demonstrates compatibility with popular monorepo tools

### Testing Performed

- Directory structure verification - Passed (all directories created correctly)
- File content verification - Passed (all files match specification exactly)
- Path dependency structure - Verified (main_app -> ui_components -> core)
- Manual testing required: `flutter pub get` in each package (requires Flutter SDK)
- Manual testing required: `flutter run --machine` in apps/main_app (requires Flutter SDK and device)

### Risks/Limitations

1. **Flutter SDK Required**: Cannot verify `flutter pub get` and `flutter run` without Flutter SDK access in this environment. User must manually verify acceptance criteria 1-5.
2. **Device Required**: Testing the app requires a connected device or emulator
3. **Path Resolution**: Relative paths in pubspec.yaml must be verified to work correctly both locally and in Docker environment
