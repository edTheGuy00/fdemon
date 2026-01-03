## Task: Testing and Documentation

**Objective**: Add comprehensive tests for the discovery feature including plugin/package detection, and update project documentation to reflect the new auto-discovery behavior with project type filtering.

**Depends on**: [03-integrate-flow](03-integrate-flow.md)

---

### Scope

- `src/core/discovery.rs`: Unit tests (if not already complete)
- `src/tui/selector.rs`: Unit tests for helper functions
- `tests/integration/discovery.rs`: Integration tests for end-to-end discovery
- `README.md`: Update usage documentation
- `CHANGELOG.md`: Add entry for new feature (if exists)

---

### Testing Strategy

#### Unit Tests (In Module Files)

Already specified in Task 01 and Task 02. Verify completion:

- [ ] `discovery.rs` tests all pass
- [ ] `selector.rs` helper function tests pass

#### Integration Tests

Create `tests/integration/discovery.rs` (or add to existing test file):

```rust
//! Integration tests for Flutter project discovery

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use flutter_demon::core::discovery::{discover_flutter_projects, DEFAULT_MAX_DEPTH};

/// Helper to create a runnable Flutter application structure
fn create_flutter_app(path: &std::path::Path, name: &str) {
    let project_dir = path.join(name);
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(
        project_dir.join("pubspec.yaml"),
        format!(r#"name: {}

dependencies:
  flutter:
    sdk: flutter

flutter:
  uses-material-design: true
"#, name),
    ).unwrap();
    fs::create_dir_all(project_dir.join("lib")).unwrap();
    fs::create_dir_all(project_dir.join("android")).unwrap();
    fs::create_dir_all(project_dir.join("ios")).unwrap();
    fs::write(
        project_dir.join("lib/main.dart"),
        "void main() {}\n",
    ).unwrap();
}

/// Helper to create a Flutter plugin structure
fn create_flutter_plugin(path: &std::path::Path, name: &str) {
    let project_dir = path.join(name);
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(
        project_dir.join("pubspec.yaml"),
        format!(r#"name: {}

dependencies:
  flutter:
    sdk: flutter

flutter:
  plugin:
    platforms:
      android:
        package: com.example.{}
        pluginClass: {}Plugin
      ios:
        pluginClass: {}Plugin
"#, name, name, name, name),
    ).unwrap();
    fs::create_dir_all(project_dir.join("lib")).unwrap();
}

/// Helper to create a Dart-only package structure
fn create_dart_package(path: &std::path::Path, name: &str) {
    let project_dir = path.join(name);
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(
        project_dir.join("pubspec.yaml"),
        format!(r#"name: {}

dependencies:
  collection: ^1.17.0
"#, name),
    ).unwrap();
    fs::create_dir_all(project_dir.join("lib")).unwrap();
}

/// Helper to create a Flutter package (no platform dirs)
fn create_flutter_package(path: &std::path::Path, name: &str) {
    let project_dir = path.join(name);
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(
        project_dir.join("pubspec.yaml"),
        format!(r#"name: {}

dependencies:
  flutter:
    sdk: flutter
"#, name),
    ).unwrap();
    fs::create_dir_all(project_dir.join("lib")).unwrap();
    // Note: NO platform directories
}

// ═══════════════════════════════════════════════════════════════
// Runnable Flutter App Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_discovery_single_runnable_app_at_root() {
    let temp = TempDir::new().unwrap();
    
    // Create a full Flutter app at root (with platform dirs)
    fs::write(temp.path().join("pubspec.yaml"), r#"
name: root_app
dependencies:
  flutter:
    sdk: flutter
flutter:
  uses-material-design: true
"#).unwrap();
    fs::create_dir_all(temp.path().join("android")).unwrap();
    fs::create_dir_all(temp.path().join("ios")).unwrap();
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    assert_eq!(result.projects.len(), 1);
    assert_eq!(result.projects[0], temp.path().to_path_buf());
}

#[test]
fn test_discovery_single_app_in_subdir() {
    let temp = TempDir::new().unwrap();
    create_flutter_app(temp.path(), "my_app");
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    assert_eq!(result.projects.len(), 1);
    assert!(result.projects[0].ends_with("my_app"));
}

#[test]
fn test_discovery_multiple_runnable_apps() {
    let temp = TempDir::new().unwrap();
    create_flutter_app(temp.path(), "app_one");
    create_flutter_app(temp.path(), "app_two");
    create_flutter_app(temp.path(), "app_three");
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    assert_eq!(result.projects.len(), 3);
    // Verify sorted order
    let names: Vec<_> = result.projects.iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap())
        .collect();
    assert_eq!(names, vec!["app_one", "app_three", "app_two"]);
}

// ═══════════════════════════════════════════════════════════════
// Plugin Detection Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_plugin_is_not_directly_runnable() {
    let temp = TempDir::new().unwrap();
    create_flutter_plugin(temp.path(), "my_plugin");
    
    // Plugin itself should not be in results
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    assert!(result.projects.is_empty());
    assert!(!result.skipped.is_empty());
    assert!(result.skipped.iter().any(|s| 
        matches!(s.project_type, ProjectType::Plugin)
    ));
}

#[test]
fn test_plugin_example_is_discovered() {
    let temp = TempDir::new().unwrap();
    
    // Create plugin
    create_flutter_plugin(temp.path(), "my_plugin");
    
    // Create runnable example inside plugin
    let plugin_dir = temp.path().join("my_plugin");
    let example_dir = plugin_dir.join("example");
    fs::create_dir_all(&example_dir).unwrap();
    fs::write(example_dir.join("pubspec.yaml"), r#"
name: my_plugin_example
dependencies:
  flutter:
    sdk: flutter
flutter:
  uses-material-design: true
"#).unwrap();
    fs::create_dir_all(example_dir.join("android")).unwrap();
    fs::create_dir_all(example_dir.join("ios")).unwrap();
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    assert_eq!(result.projects.len(), 1);
    assert!(result.projects[0].ends_with("example"));
}

#[test]
fn test_plugin_sample_dir_is_discovered() {
    let temp = TempDir::new().unwrap();
    
    // Create plugin
    create_flutter_plugin(temp.path(), "my_plugin");
    
    // Create runnable sample inside plugin (alternative to example/)
    let plugin_dir = temp.path().join("my_plugin");
    let sample_dir = plugin_dir.join("sample");
    fs::create_dir_all(&sample_dir).unwrap();
    fs::write(sample_dir.join("pubspec.yaml"), r#"
name: my_plugin_sample
dependencies:
  flutter:
    sdk: flutter
"#).unwrap();
    fs::create_dir_all(sample_dir.join("android")).unwrap();
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    assert_eq!(result.projects.len(), 1);
    assert!(result.projects[0].ends_with("sample"));
}

// ═══════════════════════════════════════════════════════════════
// Package Detection Tests (Dart-only and Flutter packages)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_dart_package_is_skipped() {
    let temp = TempDir::new().unwrap();
    
    // Create Dart-only package (no flutter dependency)
    create_dart_package(temp.path(), "dart_utils");
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    assert!(result.projects.is_empty());
    assert!(result.skipped.iter().any(|s| 
        matches!(s.project_type, ProjectType::DartPackage)
    ));
}

#[test]
fn test_flutter_package_without_platforms_is_skipped() {
    let temp = TempDir::new().unwrap();
    
    // Create Flutter package without platform directories
    create_flutter_package(temp.path(), "flutter_utils");
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    assert!(result.projects.is_empty());
    assert!(result.skipped.iter().any(|s| 
        matches!(s.project_type, ProjectType::FlutterPackage)
    ));
}

#[test]
fn test_mixed_project_types() {
    let temp = TempDir::new().unwrap();
    
    // Create various project types
    create_flutter_app(temp.path(), "runnable_app");      // Should be found
    create_flutter_plugin(temp.path(), "my_plugin");       // Should be skipped
    create_dart_package(temp.path(), "dart_utils");        // Should be skipped
    create_flutter_package(temp.path(), "flutter_utils");  // Should be skipped
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    // Only the runnable app should be in results
    assert_eq!(result.projects.len(), 1);
    assert!(result.projects[0].ends_with("runnable_app"));
    
    // Three projects should be skipped
    assert_eq!(result.skipped.len(), 3);
}

// ═══════════════════════════════════════════════════════════════
// Edge Case Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_discovery_skips_hidden_dirs() {
    let temp = TempDir::new().unwrap();
    
    // Hidden directory with app (should be skipped)
    let hidden = temp.path().join(".hidden_app");
    fs::create_dir_all(&hidden).unwrap();
    fs::write(hidden.join("pubspec.yaml"), r#"
name: hidden
dependencies:
  flutter:
    sdk: flutter
"#).unwrap();
    fs::create_dir_all(hidden.join("android")).unwrap();
    
    // Visible app (should be found)
    create_flutter_app(temp.path(), "visible_app");
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    assert_eq!(result.projects.len(), 1);
    assert!(result.projects[0].ends_with("visible_app"));
}

#[test]
fn test_discovery_skips_build_dirs() {
    let temp = TempDir::new().unwrap();
    
    // Create main app
    create_flutter_app(temp.path(), "my_app");
    
    // Create fake project in build dir (should be skipped)
    let build_dir = temp.path().join("my_app/build/fake_project");
    fs::create_dir_all(&build_dir).unwrap();
    fs::write(build_dir.join("pubspec.yaml"), r#"
name: fake
dependencies:
  flutter:
    sdk: flutter
"#).unwrap();
    fs::create_dir_all(build_dir.join("android")).unwrap();
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    assert_eq!(result.projects.len(), 1);
    assert!(result.projects[0].ends_with("my_app"));
}

#[test]
fn test_discovery_respects_depth_limit() {
    let temp = TempDir::new().unwrap();
    
    // Create app at depth 5
    let deep_path = temp.path().join("a/b/c/d/e");
    fs::create_dir_all(&deep_path).unwrap();
    fs::write(deep_path.join("pubspec.yaml"), r#"
name: deep
dependencies:
  flutter:
    sdk: flutter
"#).unwrap();
    fs::create_dir_all(deep_path.join("android")).unwrap();
    
    // Depth 3 should not find it
    let result = discover_flutter_projects(temp.path(), 3);
    assert!(result.projects.is_empty());
    
    // Depth 6 should find it
    let result = discover_flutter_projects(temp.path(), 6);
    assert_eq!(result.projects.len(), 1);
}

#[test]
fn test_discovery_no_projects() {
    let temp = TempDir::new().unwrap();
    
    // Create some non-Flutter directories
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::create_dir_all(temp.path().join("docs")).unwrap();
    fs::write(temp.path().join("README.md"), "# Test").unwrap();
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    assert!(result.projects.is_empty());
    assert!(result.skipped.is_empty()); // No pubspec.yaml files at all
    assert_eq!(result.searched_from, temp.path().to_path_buf());
}

#[test]
fn test_monorepo_with_apps_and_packages() {
    let temp = TempDir::new().unwrap();
    
    // Simulate a monorepo structure
    // apps/
    //   app1/ (runnable)
    //   app2/ (runnable)
    // packages/
    //   shared_utils/ (Dart package - skip)
    //   shared_widgets/ (Flutter package - skip)
    // plugins/
    //   my_plugin/ (plugin - skip, but check example/)
    //     example/ (runnable)
    
    // Apps
    let apps_dir = temp.path().join("apps");
    fs::create_dir_all(&apps_dir).unwrap();
    create_flutter_app(&apps_dir, "app1");
    create_flutter_app(&apps_dir, "app2");
    
    // Packages
    let packages_dir = temp.path().join("packages");
    fs::create_dir_all(&packages_dir).unwrap();
    create_dart_package(&packages_dir, "shared_utils");
    create_flutter_package(&packages_dir, "shared_widgets");
    
    // Plugin with example
    let plugins_dir = temp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    create_flutter_plugin(&plugins_dir, "my_plugin");
    
    let plugin_example = plugins_dir.join("my_plugin/example");
    fs::create_dir_all(&plugin_example).unwrap();
    fs::write(plugin_example.join("pubspec.yaml"), r#"
name: my_plugin_example
dependencies:
  flutter:
    sdk: flutter
"#).unwrap();
    fs::create_dir_all(plugin_example.join("ios")).unwrap();
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    // Should find: app1, app2, my_plugin/example
    assert_eq!(result.projects.len(), 3);
    
    let paths: Vec<_> = result.projects.iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    
    assert!(paths.iter().any(|p| p.contains("app1")));
    assert!(paths.iter().any(|p| p.contains("app2")));
    assert!(paths.iter().any(|p| p.contains("example")));
}

#[test]
fn test_malformed_pubspec_is_skipped() {
    let temp = TempDir::new().unwrap();
    
    // Create directory with malformed pubspec
    let bad_project = temp.path().join("bad_project");
    fs::create_dir_all(&bad_project).unwrap();
    fs::write(bad_project.join("pubspec.yaml"), "this is not valid yaml: [").unwrap();
    fs::create_dir_all(bad_project.join("android")).unwrap();
    
    // Create valid app
    create_flutter_app(temp.path(), "good_app");
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    // Should find the good app, skip the bad one gracefully
    assert_eq!(result.projects.len(), 1);
    assert!(result.projects[0].ends_with("good_app"));
}

#[test]
fn test_realistic_flutter_app_structure() {
    let temp = TempDir::new().unwrap();
    let app = temp.path().join("my_flutter_app");
    
    // Create realistic Flutter project structure
    fs::create_dir_all(&app).unwrap();
    fs::write(app.join("pubspec.yaml"), r#"
name: my_flutter_app
description: A test Flutter app
version: 1.0.0+1

environment:
  sdk: ^3.0.0

dependencies:
  flutter:
    sdk: flutter
  cupertino_icons: ^1.0.8

dev_dependencies:
  flutter_test:
    sdk: flutter

flutter:
  uses-material-design: true
"#).unwrap();
    
    fs::create_dir_all(app.join("lib")).unwrap();
    fs::create_dir_all(app.join("test")).unwrap();
    fs::create_dir_all(app.join("android")).unwrap();
    fs::create_dir_all(app.join("ios")).unwrap();
    fs::create_dir_all(app.join("macos")).unwrap();
    fs::create_dir_all(app.join("web")).unwrap();
    fs::create_dir_all(app.join(".dart_tool")).unwrap();
    
    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
    
    assert_eq!(result.projects.len(), 1);
    assert_eq!(result.projects[0], app);
}
```

---

### Documentation Updates

#### README.md

Add a section explaining the discovery behavior:

```markdown
## Usage

### Running Flutter Demon

You can run Flutter Demon in several ways:

#### From a Flutter App Directory
```bash
cd /path/to/my_flutter_app
flutter-demon
```

#### With an Explicit Path
```bash
flutter-demon /path/to/my_flutter_app
```

#### Auto-Discovery Mode
If you run Flutter Demon from a directory that isn't a runnable Flutter app,
it will automatically search subdirectories for runnable Flutter projects:

```bash
cd /path/to/workspace  # Contains multiple Flutter projects
flutter-demon
```

If multiple runnable projects are found, you'll see a selection menu:

```
Multiple Flutter projects found in:
/path/to/workspace

Select a project:

  [1] app_one
  [2] app_two
  [3] my_plugin/example

Enter number (1-3) or 'q' to quit:
```

Press the number key to select a project, or 'q' to cancel.

### Project Type Detection

Flutter Demon intelligently detects different project types:

| Type | Runnable? | What Happens |
|------|-----------|--------------|
| **Flutter App** | ✅ Yes | Runs directly |
| **Flutter Plugin** | ❌ No | Searches `example/` subdirectory |
| **Flutter Package** | ❌ No | Skipped (no platform directories) |
| **Dart Package** | ❌ No | Skipped (no Flutter dependency) |

#### Working with Plugins

If you're developing a Flutter plugin, run Flutter Demon from the plugin directory
and it will automatically find and use the `example/` project:

```bash
cd /path/to/my_plugin
flutter-demon

# Output:
# 📦 Detected Flutter plugin at: /path/to/my_plugin
#    Plugins cannot be run directly. Searching for runnable examples...
#
# ✅ Found Flutter project: /path/to/my_plugin/example
```
```

---

### Acceptance Criteria

1. All unit tests in `discovery.rs` pass (including plugin/package detection)
2. All unit tests in `selector.rs` pass
3. All integration tests pass
4. `cargo test` completes successfully
5. README.md includes discovery feature documentation with project type table
6. No regressions in existing functionality
7. Plugin detection tests cover `example/` and `sample/` directories
8. Dart package detection tests verify correct skipping behavior
9. Flutter package (no platform dirs) detection tests pass
10. Monorepo scenario test passes with mixed project types

---

### Test Coverage Targets

| Module | Coverage Goal |
|--------|---------------|
| `core/discovery.rs` | 90%+ |
| `tui/selector.rs` | 80%+ (helper functions) |
| Integration tests | All major flows covered |

---

## Manual Testing Checklist

Perform these manual tests before marking complete:

### Basic Discovery
- [ ] Run `flutter-demon` from a Flutter app dir → TUI starts
- [ ] Run `flutter-demon` from parent of single Flutter app → Auto-selects
- [ ] Run `flutter-demon` from dir with 2+ Flutter apps → Shows selector
- [ ] Press number to select project → TUI starts with that project
- [ ] Press 'q' in selector → Clean exit
- [ ] Press Ctrl+C in selector → Clean exit
- [ ] Run from dir with no Flutter projects → Shows error message
- [ ] Run `flutter-demon /explicit/path` → Works with explicit path
- [ ] Check logs for discovery information

### Plugin Detection
- [ ] Run from Flutter plugin directory → Detects plugin, finds example/
- [ ] Run from plugin with no example/ → Shows helpful error
- [ ] Plugin's example/ app starts correctly in TUI

### Package Detection
- [ ] Run from Dart-only package → Shows "Dart package not runnable" message
- [ ] Run from Flutter package (no platforms) → Shows "no platform directories" message

### Monorepo Scenarios
- [ ] Run from monorepo root with apps/ and packages/ → Only shows runnable apps
- [ ] Plugin examples in monorepo are discovered correctly

---

### Documentation Files to Update

| File | Changes |
|------|---------|
| `README.md` | Add discovery usage section |
| `CHANGELOG.md` | Add feature entry (if file exists) |
| `documents/plans/features/initial-project-setup/PLAN.md` | Add Phase 1.1 reference |

---

### Notes

- Integration tests use `tempfile` crate for isolated test directories
- Tests should clean up after themselves (TempDir handles this)
- Focus on testing discovery logic, not full TUI interaction
- Manual testing required for full end-to-end validation
- Consider adding CI workflow for automated testing (future)
- **Test helpers create realistic project structures** (pubspec.yaml + platform dirs)
- **Plugin tests verify example/ and sample/ discovery**
- **Package tests verify correct skipping of non-runnable projects**
- **Monorepo test covers realistic multi-project workspace**