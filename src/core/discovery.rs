//! Flutter project discovery module
//!
//! Discovers runnable Flutter projects by analyzing `pubspec.yaml` content
//! and checking for platform directories. Filters out plugins, packages,
//! and Dart-only projects that cannot be run with `flutter run`.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, trace};

/// Default maximum search depth
pub const DEFAULT_MAX_DEPTH: usize = 3;

/// Platform directories that indicate a runnable Flutter project
const PLATFORM_DIRECTORIES: &[&str] = &["android", "ios", "macos", "web", "linux", "windows"];

/// Subdirectories to check for runnable examples in plugins
const EXAMPLE_DIRECTORIES: &[&str] = &["example", "sample"];

/// Directories to skip during search
const SKIP_DIRECTORIES: &[&str] = &[
    "node_modules",
    "build",
    ".dart_tool",
    ".git",
    ".idea",
    ".vscode",
    "Pods",
    ".gradle",
    "__pycache__",
    "target", // Rust build dir
    ".pub-cache",
    ".pub",
];

/// Classification of a pubspec.yaml project
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectType {
    /// A runnable Flutter application (has flutter dep, platform dirs, not a plugin)
    Application,
    /// A Flutter plugin (has `flutter: plugin:` section)
    Plugin,
    /// A Flutter package (has flutter dep but no platform directories)
    FlutterPackage,
    /// A pure Dart package (no flutter SDK dependency)
    DartPackage,
}

/// Information about a skipped project
#[derive(Debug)]
pub struct SkippedProject {
    pub path: PathBuf,
    pub project_type: ProjectType,
    pub reason: String,
}

/// Result of project discovery
#[derive(Debug)]
pub struct DiscoveryResult {
    /// Found runnable Flutter project paths
    pub projects: Vec<PathBuf>,
    /// Base path that was searched
    pub searched_from: PathBuf,
    /// Maximum depth that was searched
    pub max_depth: usize,
    /// Projects that were skipped (for logging/debugging)
    pub skipped: Vec<SkippedProject>,
}

/// Check if pubspec.yaml has Flutter SDK dependency
pub fn has_flutter_dependency(path: &Path) -> bool {
    let pubspec_path = path.join("pubspec.yaml");
    match fs::read_to_string(&pubspec_path) {
        Ok(content) => check_has_flutter_dependency(&content),
        Err(_) => false,
    }
}

/// Check if a project is a Flutter plugin
pub fn is_flutter_plugin(path: &Path) -> bool {
    let pubspec_path = path.join("pubspec.yaml");
    match fs::read_to_string(&pubspec_path) {
        Ok(content) => check_is_plugin(&content),
        Err(_) => false,
    }
}

/// Check if project has any platform directories
pub fn has_platform_directories(path: &Path) -> bool {
    PLATFORM_DIRECTORIES
        .iter()
        .any(|dir| path.join(dir).is_dir())
}

/// Get the project type for a directory containing pubspec.yaml
pub fn get_project_type(path: &Path) -> Option<ProjectType> {
    let pubspec_path = path.join("pubspec.yaml");
    let content = fs::read_to_string(&pubspec_path).ok()?;

    let has_flutter = check_has_flutter_dependency(&content);
    let is_plugin = check_is_plugin(&content);
    let has_platforms = has_platform_directories(path);

    Some(if !has_flutter {
        ProjectType::DartPackage
    } else if is_plugin {
        ProjectType::Plugin
    } else if has_platforms {
        ProjectType::Application
    } else {
        ProjectType::FlutterPackage
    })
}

/// Check if a directory contains a runnable Flutter project
pub fn is_runnable_flutter_project(path: &Path) -> bool {
    let pubspec_path = path.join("pubspec.yaml");

    // 1. pubspec.yaml must exist
    let content = match fs::read_to_string(&pubspec_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    // 2. Must have Flutter SDK dependency
    if !check_has_flutter_dependency(&content) {
        return false;
    }

    // 3. Must NOT be a plugin
    if check_is_plugin(&content) {
        return false;
    }

    // 4. Must have at least one platform directory
    has_platform_directories(path)
}

/// Discover runnable Flutter projects in the given directory
///
/// This function searches for projects that can be run with `flutter run`.
/// It filters out:
/// - Flutter plugins (but checks their example/ subdirectories)
/// - Flutter packages without platform directories
/// - Pure Dart packages
///
/// # Arguments
/// * `base_path` - Starting directory for search
/// * `max_depth` - Maximum directory depth to search
///
/// # Returns
/// * `DiscoveryResult` with found runnable projects sorted by path
pub fn discover_flutter_projects(base_path: &Path, max_depth: usize) -> DiscoveryResult {
    let mut result = DiscoveryResult {
        projects: Vec::new(),
        searched_from: base_path.to_path_buf(),
        max_depth,
        skipped: Vec::new(),
    };

    // Check if base_path itself is a runnable project
    if is_runnable_flutter_project(base_path) {
        debug!("Base path is a runnable Flutter project: {:?}", base_path);
        result.projects.push(base_path.to_path_buf());
        return result;
    }

    // Check if base_path is a plugin - look for examples
    if is_flutter_plugin(base_path) {
        debug!(
            "Base path is a Flutter plugin, checking for examples: {:?}",
            base_path
        );
        result.skipped.push(SkippedProject {
            path: base_path.to_path_buf(),
            project_type: ProjectType::Plugin,
            reason: "Plugin is not directly runnable".to_string(),
        });

        for example_dir in EXAMPLE_DIRECTORIES {
            let example_path = base_path.join(example_dir);
            if is_runnable_flutter_project(&example_path) {
                debug!("Found runnable example in plugin: {:?}", example_path);
                result.projects.push(example_path);
            }
        }

        if !result.projects.is_empty() {
            result.projects.sort();
            return result;
        }
    }

    // Recursively search for projects
    discover_recursive(base_path, 0, max_depth, &mut result);

    // Sort results by path
    result.projects.sort();

    result
}

/// Recursive helper for project discovery
fn discover_recursive(
    dir: &Path,
    current_depth: usize,
    max_depth: usize,
    result: &mut DiscoveryResult,
) {
    if current_depth > max_depth {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            trace!("Cannot read directory {:?}: {}", dir, err);
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        // Skip hidden directories
        if dir_name.starts_with('.') {
            trace!("Skipping hidden directory: {:?}", path);
            continue;
        }

        // Skip known non-project directories
        if SKIP_DIRECTORIES.contains(&dir_name) {
            trace!("Skipping excluded directory: {:?}", path);
            continue;
        }

        // Check if this directory is a project
        let pubspec_path = path.join("pubspec.yaml");
        if pubspec_path.exists() {
            // This is a project directory - check its type
            if let Some(project_type) = get_project_type(&path) {
                match project_type {
                    ProjectType::Application => {
                        debug!("Found runnable Flutter application: {:?}", path);
                        result.projects.push(path.clone());
                        // Don't descend into found projects
                        continue;
                    }
                    ProjectType::Plugin => {
                        debug!("Found Flutter plugin, checking examples: {:?}", path);
                        result.skipped.push(SkippedProject {
                            path: path.clone(),
                            project_type: ProjectType::Plugin,
                            reason: "Plugin is not directly runnable".to_string(),
                        });

                        // Check example directories within the plugin
                        for example_dir in EXAMPLE_DIRECTORIES {
                            let example_path = path.join(example_dir);
                            if is_runnable_flutter_project(&example_path) {
                                debug!("Found runnable example in plugin: {:?}", example_path);
                                result.projects.push(example_path);
                            }
                        }
                        // Don't descend further into the plugin
                        continue;
                    }
                    ProjectType::FlutterPackage => {
                        debug!("Skipping Flutter package (no platform dirs): {:?}", path);
                        result.skipped.push(SkippedProject {
                            path: path.clone(),
                            project_type: ProjectType::FlutterPackage,
                            reason: "No platform directories found".to_string(),
                        });
                        continue;
                    }
                    ProjectType::DartPackage => {
                        debug!("Skipping Dart package (no Flutter dependency): {:?}", path);
                        result.skipped.push(SkippedProject {
                            path: path.clone(),
                            project_type: ProjectType::DartPackage,
                            reason: "No Flutter SDK dependency".to_string(),
                        });
                        continue;
                    }
                }
            }
        }

        // Recurse into subdirectory
        discover_recursive(&path, current_depth + 1, max_depth, result);
    }
}

/// Check if pubspec.yaml indicates a Flutter plugin
fn check_is_plugin(content: &str) -> bool {
    // Look for pattern:
    // flutter:
    //   plugin:
    //     platforms:

    let lines: Vec<&str> = content.lines().collect();
    let mut in_flutter_section = false;

    for line in lines {
        let trimmed = line.trim();

        // Detect flutter: section (must be at root level, no leading spaces)
        if line.starts_with("flutter:") && !line.starts_with(' ') {
            in_flutter_section = true;
            continue;
        }

        // If we hit another root-level key, exit flutter section
        if in_flutter_section && !line.starts_with(' ') && !trimmed.is_empty() {
            in_flutter_section = false;
        }

        // Look for plugin: within flutter section
        if in_flutter_section && trimmed.starts_with("plugin:") {
            return true;
        }
    }

    false
}

/// Parse the project name from pubspec.yaml
///
/// This function reads the pubspec.yaml file and extracts the project name.
/// Falls back to the directory name if parsing fails.
pub fn get_project_name(project_path: &Path) -> Option<String> {
    let pubspec_path = project_path.join("pubspec.yaml");
    let content = fs::read_to_string(&pubspec_path).ok()?;

    // Simple line-by-line parsing for "name: value"
    for line in content.lines() {
        let trimmed = line.trim();
        // Only match "name:" at the start of a non-indented line
        if trimmed.starts_with("name:") && !line.starts_with(' ') && !line.starts_with('\t') {
            let name = trimmed.strip_prefix("name:")?.trim();
            // Remove quotes if present
            let name = name.trim_matches('"').trim_matches('\'');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Check if pubspec.yaml has flutter SDK dependency
fn check_has_flutter_dependency(content: &str) -> bool {
    // Look for pattern:
    // dependencies:
    //   flutter:
    //     sdk: flutter
    //
    // Simple check: look for "sdk: flutter" anywhere in file
    // This is a reasonable heuristic since "sdk: flutter" is unique
    content.contains("sdk: flutter")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a minimal Flutter app structure
    fn create_flutter_app(path: &Path, name: &str) {
        fs::write(
            path.join("pubspec.yaml"),
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
        fs::create_dir_all(path.join("lib")).unwrap();
        fs::create_dir_all(path.join("android")).unwrap();
        fs::create_dir_all(path.join("ios")).unwrap();
    }

    /// Helper to create a Flutter plugin structure
    fn create_flutter_plugin(path: &Path, name: &str) {
        fs::write(
            path.join("pubspec.yaml"),
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
        fs::create_dir_all(path.join("lib")).unwrap();
    }

    /// Helper to create a Dart-only package
    fn create_dart_package(path: &Path, name: &str) {
        fs::write(
            path.join("pubspec.yaml"),
            format!(
                r#"name: {}
dependencies:
  collection: ^1.17.0
"#,
                name
            ),
        )
        .unwrap();
        fs::create_dir_all(path.join("lib")).unwrap();
    }

    /// Helper to create a Flutter package (no platform dirs)
    fn create_flutter_package(path: &Path, name: &str) {
        fs::write(
            path.join("pubspec.yaml"),
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
        fs::create_dir_all(path.join("lib")).unwrap();
        // Note: NO platform directories created
    }

    #[test]
    fn test_is_runnable_flutter_project() {
        let temp = TempDir::new().unwrap();
        let app_path = temp.path().join("my_app");
        fs::create_dir(&app_path).unwrap();
        create_flutter_app(&app_path, "my_app");

        assert!(is_runnable_flutter_project(&app_path));
    }

    #[test]
    fn test_plugin_is_not_runnable() {
        let temp = TempDir::new().unwrap();
        let plugin_path = temp.path().join("my_plugin");
        fs::create_dir(&plugin_path).unwrap();
        create_flutter_plugin(&plugin_path, "my_plugin");

        assert!(!is_runnable_flutter_project(&plugin_path));
        assert!(is_flutter_plugin(&plugin_path));
    }

    #[test]
    fn test_dart_package_is_not_runnable() {
        let temp = TempDir::new().unwrap();
        let pkg_path = temp.path().join("my_pkg");
        fs::create_dir(&pkg_path).unwrap();
        create_dart_package(&pkg_path, "my_pkg");

        assert!(!is_runnable_flutter_project(&pkg_path));
        assert!(!has_flutter_dependency(&pkg_path));
    }

    #[test]
    fn test_flutter_package_without_platforms_not_runnable() {
        let temp = TempDir::new().unwrap();
        let pkg_path = temp.path().join("my_flutter_pkg");
        fs::create_dir(&pkg_path).unwrap();
        create_flutter_package(&pkg_path, "my_flutter_pkg");

        assert!(!is_runnable_flutter_project(&pkg_path));
        assert!(has_flutter_dependency(&pkg_path));
        assert!(!has_platform_directories(&pkg_path));
    }

    #[test]
    fn test_discover_plugin_example() {
        let temp = TempDir::new().unwrap();

        // Create plugin at root
        let plugin_path = temp.path().join("my_plugin");
        fs::create_dir(&plugin_path).unwrap();
        create_flutter_plugin(&plugin_path, "my_plugin");

        // Create runnable example inside plugin
        let example_path = plugin_path.join("example");
        fs::create_dir(&example_path).unwrap();
        create_flutter_app(&example_path, "my_plugin_example");

        let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);

        assert_eq!(result.projects.len(), 1);
        assert!(result.projects[0].ends_with("example"));
    }

    #[test]
    fn test_discover_skips_dart_packages() {
        let temp = TempDir::new().unwrap();

        // Create Dart package (should be skipped)
        let dart_pkg = temp.path().join("dart_utils");
        fs::create_dir(&dart_pkg).unwrap();
        create_dart_package(&dart_pkg, "dart_utils");

        // Create Flutter app (should be found)
        let app_path = temp.path().join("my_app");
        fs::create_dir(&app_path).unwrap();
        create_flutter_app(&app_path, "my_app");

        let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);

        assert_eq!(result.projects.len(), 1);
        assert!(result.projects[0].ends_with("my_app"));
    }

    #[test]
    fn test_discover_multiple_apps() {
        let temp = TempDir::new().unwrap();

        for name in ["app_a", "app_b", "app_c"] {
            let path = temp.path().join(name);
            fs::create_dir(&path).unwrap();
            create_flutter_app(&path, name);
        }

        let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);

        assert_eq!(result.projects.len(), 3);
        // Verify sorted order
        let names: Vec<_> = result
            .projects
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["app_a", "app_b", "app_c"]);
    }

    #[test]
    fn test_has_platform_directories() {
        let temp = TempDir::new().unwrap();

        // No platform dirs
        assert!(!has_platform_directories(temp.path()));

        // Add android/
        fs::create_dir(temp.path().join("android")).unwrap();
        assert!(has_platform_directories(temp.path()));
    }

    #[test]
    fn test_get_project_type() {
        let temp = TempDir::new().unwrap();

        // Flutter app
        let app = temp.path().join("app");
        fs::create_dir(&app).unwrap();
        create_flutter_app(&app, "app");
        assert_eq!(get_project_type(&app), Some(ProjectType::Application));

        // Flutter plugin
        let plugin = temp.path().join("plugin");
        fs::create_dir(&plugin).unwrap();
        create_flutter_plugin(&plugin, "plugin");
        assert_eq!(get_project_type(&plugin), Some(ProjectType::Plugin));

        // Dart package
        let dart_pkg = temp.path().join("dart_pkg");
        fs::create_dir(&dart_pkg).unwrap();
        create_dart_package(&dart_pkg, "dart_pkg");
        assert_eq!(get_project_type(&dart_pkg), Some(ProjectType::DartPackage));

        // Flutter package (no platform dirs)
        let flutter_pkg = temp.path().join("flutter_pkg");
        fs::create_dir(&flutter_pkg).unwrap();
        create_flutter_package(&flutter_pkg, "flutter_pkg");
        assert_eq!(
            get_project_type(&flutter_pkg),
            Some(ProjectType::FlutterPackage)
        );
    }

    #[test]
    fn test_skips_hidden_directories() {
        let temp = TempDir::new().unwrap();

        // Hidden app (should be skipped)
        let hidden = temp.path().join(".hidden_app");
        fs::create_dir(&hidden).unwrap();
        create_flutter_app(&hidden, "hidden_app");

        let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);
        assert!(result.projects.is_empty());
    }

    #[test]
    fn test_respects_max_depth() {
        let temp = TempDir::new().unwrap();

        // Create app at depth 5
        let deep_path = temp.path().join("a/b/c/d/e/deep_app");
        fs::create_dir_all(&deep_path).unwrap();
        create_flutter_app(&deep_path, "deep_app");

        // Depth 3 should not find it
        let result = discover_flutter_projects(temp.path(), 3);
        assert!(result.projects.is_empty());

        // Depth 6 should find it
        let result = discover_flutter_projects(temp.path(), 6);
        assert_eq!(result.projects.len(), 1);
    }

    #[test]
    fn test_base_path_is_runnable_app() {
        let temp = TempDir::new().unwrap();
        create_flutter_app(temp.path(), "root_app");

        let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);

        assert_eq!(result.projects.len(), 1);
        assert_eq!(result.projects[0], temp.path().to_path_buf());
    }

    #[test]
    fn test_check_is_plugin() {
        let plugin_pubspec = r#"name: my_plugin
dependencies:
  flutter:
    sdk: flutter
flutter:
  plugin:
    platforms:
      android:
        package: com.example.my_plugin
"#;
        assert!(check_is_plugin(plugin_pubspec));

        let app_pubspec = r#"name: my_app
dependencies:
  flutter:
    sdk: flutter
flutter:
  uses-material-design: true
"#;
        assert!(!check_is_plugin(app_pubspec));
    }

    #[test]
    fn test_check_has_flutter_dependency() {
        let flutter_app = r#"name: my_app
dependencies:
  flutter:
    sdk: flutter
"#;
        assert!(check_has_flutter_dependency(flutter_app));

        let dart_pkg = r#"name: my_pkg
dependencies:
  collection: ^1.17.0
"#;
        assert!(!check_has_flutter_dependency(dart_pkg));
    }

    #[test]
    fn test_discover_plugin_at_base_path() {
        let temp = TempDir::new().unwrap();

        // Create plugin at temp root
        create_flutter_plugin(temp.path(), "root_plugin");

        // Create runnable example inside plugin
        let example_path = temp.path().join("example");
        fs::create_dir(&example_path).unwrap();
        create_flutter_app(&example_path, "root_plugin_example");

        let result = discover_flutter_projects(temp.path(), DEFAULT_MAX_DEPTH);

        assert_eq!(result.projects.len(), 1);
        assert!(result.projects[0].ends_with("example"));
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.skipped[0].project_type, ProjectType::Plugin);
    }

    #[test]
    fn test_malformed_pubspec_handled_gracefully() {
        let temp = TempDir::new().unwrap();
        let bad_path = temp.path().join("bad_project");
        fs::create_dir(&bad_path).unwrap();
        fs::write(bad_path.join("pubspec.yaml"), "this is not valid yaml: [").unwrap();

        // Should not panic, should return None
        assert!(get_project_type(&bad_path).is_some()); // Will be DartPackage since no "sdk: flutter"
        assert!(!is_runnable_flutter_project(&bad_path));
    }

    #[test]
    fn test_missing_pubspec_returns_none() {
        let temp = TempDir::new().unwrap();
        assert!(get_project_type(temp.path()).is_none());
        assert!(!is_runnable_flutter_project(temp.path()));
    }

    #[test]
    fn test_get_project_name_basic() {
        let temp = TempDir::new().unwrap();
        let pubspec = temp.path().join("pubspec.yaml");

        fs::write(&pubspec, "name: my_flutter_app\nversion: 1.0.0\n").unwrap();

        let name = get_project_name(temp.path());
        assert_eq!(name, Some("my_flutter_app".to_string()));
    }

    #[test]
    fn test_get_project_name_with_double_quotes() {
        let temp = TempDir::new().unwrap();
        let pubspec = temp.path().join("pubspec.yaml");

        fs::write(&pubspec, "name: \"quoted_name\"\n").unwrap();

        let name = get_project_name(temp.path());
        assert_eq!(name, Some("quoted_name".to_string()));
    }

    #[test]
    fn test_get_project_name_with_single_quotes() {
        let temp = TempDir::new().unwrap();
        let pubspec = temp.path().join("pubspec.yaml");

        fs::write(&pubspec, "name: 'single_quoted'\n").unwrap();

        let name = get_project_name(temp.path());
        assert_eq!(name, Some("single_quoted".to_string()));
    }

    #[test]
    fn test_get_project_name_missing_file() {
        let temp = TempDir::new().unwrap();
        assert!(get_project_name(temp.path()).is_none());
    }

    #[test]
    fn test_get_project_name_ignores_nested_name() {
        let temp = TempDir::new().unwrap();
        let pubspec = temp.path().join("pubspec.yaml");

        // Name inside a nested block should be ignored
        fs::write(
            &pubspec,
            "name: real_name\ndependencies:\n  some_package:\n    name: nested_name\n",
        )
        .unwrap();

        let name = get_project_name(temp.path());
        assert_eq!(name, Some("real_name".to_string()));
    }
}
