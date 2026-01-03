//! Integration tests for Flutter project discovery

use std::fs;
use std::path::Path;
use tempfile::TempDir;

use flutter_demon::core::{discover_flutter_projects, ProjectType, DEFAULT_MAX_DEPTH};

/// Helper to create a runnable Flutter application structure
fn create_flutter_app(path: &Path, name: &str) {
    let project_dir = path.join(name);
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(
        project_dir.join("pubspec.yaml"),
        format!(
            r#"name: {}

dependencies:
  flutter:
    sdk: flutter

flutter:
  uses-material-design: true
"#,
            name
        ),
    )
    .unwrap();
    fs::create_dir_all(project_dir.join("lib")).unwrap();
    fs::create_dir_all(project_dir.join("android")).unwrap();
    fs::create_dir_all(project_dir.join("ios")).unwrap();
    fs::write(project_dir.join("lib/main.dart"), "void main() {}\n").unwrap();
}

/// Helper to create a Flutter plugin structure
fn create_flutter_plugin(path: &Path, name: &str) {
    let project_dir = path.join(name);
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(
        project_dir.join("pubspec.yaml"),
        format!(
            r#"name: {}

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
"#,
            name, name, name, name
        ),
    )
    .unwrap();
    fs::create_dir_all(project_dir.join("lib")).unwrap();
}

/// Helper to create a Dart-only package structure
fn create_dart_package(path: &Path, name: &str) {
    let project_dir = path.join(name);
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(
        project_dir.join("pubspec.yaml"),
        format!(
            r#"name: {}

dependencies:
  collection: ^1.17.0
"#,
            name
        ),
    )
    .unwrap();
    fs::create_dir_all(project_dir.join("lib")).unwrap();
}

/// Helper to create a Flutter package (no platform dirs)
fn create_flutter_package(path: &Path, name: &str) {
    let project_dir = path.join(name);
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(
        project_dir.join("pubspec.yaml"),
        format!(
            r#"name: {}

dependencies:
  flutter:
    sdk: flutter
"#,
            name
        ),
    )
    .unwrap();
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
    fs::write(
        temp.path().join("pubspec.yaml"),
        r#"
name: root_app
dependencies:
  flutter:
    sdk: flutter
flutter:
  uses-material-design: true
"#,
    )
    .unwrap();
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
    let names: Vec<_> = result
        .projects
        .iter()
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
    assert!(result
        .skipped
        .iter()
        .any(|s| matches!(s.project_type, ProjectType::Plugin)));
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
    fs::write(
        example_dir.join("pubspec.yaml"),
        r#"
name: my_plugin_example
dependencies:
  flutter:
    sdk: flutter
flutter:
  uses-material-design: true
"#,
    )
    .unwrap();
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
    fs::write(
        sample_dir.join("pubspec.yaml"),
        r#"
name: my_plugin_sample
dependencies:
  flutter:
    sdk: flutter
"#,
    )
    .unwrap();
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
    assert!(result
        .skipped
        .iter()
        .any(|s| matches!(s.project_type, ProjectType::DartPackage)));
}

#[test]
fn test_flutter_package_without_platforms_is_skipped() {
    let temp = TempDir::new().unwrap();

    // Create Flutter package without platform directories
    create_flutter_package(temp.path(), "flutter_utils");

    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);

    assert!(result.projects.is_empty());
    assert!(result
        .skipped
        .iter()
        .any(|s| matches!(s.project_type, ProjectType::FlutterPackage)));
}

#[test]
fn test_mixed_project_types() {
    let temp = TempDir::new().unwrap();

    // Create various project types
    create_flutter_app(temp.path(), "runnable_app"); // Should be found
    create_flutter_plugin(temp.path(), "my_plugin"); // Should be skipped
    create_dart_package(temp.path(), "dart_utils"); // Should be skipped
    create_flutter_package(temp.path(), "flutter_utils"); // Should be skipped

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
    fs::write(
        hidden.join("pubspec.yaml"),
        r#"
name: hidden
dependencies:
  flutter:
    sdk: flutter
"#,
    )
    .unwrap();
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
    fs::write(
        build_dir.join("pubspec.yaml"),
        r#"
name: fake
dependencies:
  flutter:
    sdk: flutter
"#,
    )
    .unwrap();
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
    fs::write(
        deep_path.join("pubspec.yaml"),
        r#"
name: deep
dependencies:
  flutter:
    sdk: flutter
"#,
    )
    .unwrap();
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
    fs::write(
        plugin_example.join("pubspec.yaml"),
        r#"
name: my_plugin_example
dependencies:
  flutter:
    sdk: flutter
"#,
    )
    .unwrap();
    fs::create_dir_all(plugin_example.join("ios")).unwrap();

    let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);

    // Should find: app1, app2, my_plugin/example
    assert_eq!(result.projects.len(), 3);

    let paths: Vec<_> = result
        .projects
        .iter()
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
    fs::write(
        bad_project.join("pubspec.yaml"),
        "this is not valid yaml: [",
    )
    .unwrap();
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
    fs::write(
        app.join("pubspec.yaml"),
        r#"
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
"#,
    )
    .unwrap();

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
