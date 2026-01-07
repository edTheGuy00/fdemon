## Task: Create plugin_with_example Flutter Fixture

**Objective**: Create a Flutter plugin package with an example app to test fdemon's handling of plugin project structures, particularly the common pattern of running the example app.

**Depends on**: 03-simple-app-fixture (reuses patterns)

### Scope

- `tests/fixtures/plugin_with_example/`: **NEW** - Flutter plugin with example app

### Details

Create a Flutter plugin structure that:
1. Has a minimal plugin package at the root
2. Contains an example/ directory with runnable app
3. Tests fdemon's project discovery for plugin structures
4. Validates that fdemon can run from plugin root or example/

#### Directory Structure

```
tests/fixtures/plugin_with_example/
├── pubspec.yaml                  # Plugin package
├── lib/
│   └── test_plugin.dart          # Plugin implementation
├── example/
│   ├── pubspec.yaml              # Example app
│   ├── lib/
│   │   └── main.dart             # Example app entry
│   └── .gitignore
├── test/
│   └── test_plugin_test.dart
└── .gitignore
```

#### Root pubspec.yaml (Plugin)

```yaml
name: test_plugin
description: Test plugin for E2E testing
version: 0.0.1
publish_to: 'none'

environment:
  sdk: '>=3.0.0 <4.0.0'
  flutter: ">=3.0.0"

dependencies:
  flutter:
    sdk: flutter

dev_dependencies:
  flutter_test:
    sdk: flutter

flutter:
  plugin:
    platforms: {}  # No platform implementations needed
```

#### lib/test_plugin.dart

```dart
/// Test plugin for E2E testing.
///
/// This is a minimal plugin with no platform-specific code.
library test_plugin;

class TestPlugin {
  static String get platformVersion => 'Test Platform 1.0';

  static void logMessage(String message) {
    print('[TEST_PLUGIN] $message');
  }
}
```

#### example/pubspec.yaml

```yaml
name: test_plugin_example
description: Example app for test_plugin
publish_to: 'none'

version: 1.0.0+1

environment:
  sdk: '>=3.0.0 <4.0.0'

dependencies:
  flutter:
    sdk: flutter
  test_plugin:
    path: ../

dev_dependencies:
  flutter_test:
    sdk: flutter

flutter:
  uses-material-design: false
```

#### example/lib/main.dart

```dart
import 'package:flutter/widgets.dart';
import 'package:test_plugin/test_plugin.dart';

void main() {
  debugPrint('[FDEMON_TEST] plugin_example starting');
  TestPlugin.logMessage('Plugin loaded');
  runApp(const PluginExampleApp());
}

class PluginExampleApp extends StatelessWidget {
  const PluginExampleApp({super.key});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Text(
        'Plugin Example: ${TestPlugin.platformVersion}',
        textDirection: TextDirection.ltr,
        style: const TextStyle(fontSize: 20),
      ),
    );
  }
}
```

#### Key Considerations

1. **Plugin Structure**:
   - Root is a plugin package (has `flutter.plugin` in pubspec)
   - Example is a regular app that depends on the plugin
   - This matches common open-source plugin patterns

2. **Project Discovery**:
   - fdemon should detect this is a plugin
   - Should offer to run the example app
   - Or run from example/ directly

3. **Minimal Dependencies**:
   - No platform-specific plugin code
   - No native dependencies
   - Fast to compile

4. **Path Dependencies**:
   - Example uses `path: ../` to reference parent plugin
   - Tests fdemon's handling of workspace dependencies

### Acceptance Criteria

1. `flutter pub get` succeeds in root directory
2. `flutter pub get` succeeds in example/ directory
3. `flutter run --machine` in example/ starts the app
4. App logs `[FDEMON_TEST] plugin_example starting`
5. App displays plugin version string
6. fdemon project discovery identifies this as a plugin project
7. fdemon can run the example app from root (if supported)

### Testing

```bash
# Test plugin package
cd tests/fixtures/plugin_with_example
flutter pub get
flutter analyze

# Test example app
cd example
flutter pub get
flutter run --machine

# Test fdemon discovery (once implemented)
cd tests/fixtures/plugin_with_example
fdemon  # Should detect plugin structure
```

### Notes

- Plugin fixture tests a common real-world structure
- Many plugins have examples that developers run frequently
- Future: Could add platform-specific code for deeper testing
- Consider testing nested plugin (plugin within plugin) later

---

## Completion Summary

**Status:** Not Started
