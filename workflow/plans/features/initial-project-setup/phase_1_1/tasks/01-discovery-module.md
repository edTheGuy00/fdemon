## Task: Discovery Module

**Objective**: Create a module that discovers **runnable** Flutter projects by analyzing `pubspec.yaml` content and checking for platform directories. Filter out plugins, packages, and Dart-only projects that cannot be run with `flutter run`.

**Depends on**: Phase 1 complete (existing project structure)

---

### Scope

- `src/core/discovery.rs`: New module for Flutter project discovery
- `src/core/mod.rs`: Add `pub mod discovery;` export

---

### Implementation Details

#### Project Type Classification

```rust
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
```

#### Public API

```rust
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

/// Information about a skipped project
#[derive(Debug)]
pub struct SkippedProject {
    pub path: PathBuf,
    pub project_type: ProjectType,
    pub reason: String,
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
/// * `max_depth` - Maximum directory depth to search (default: 3)
/// 
/// # Returns
/// * `DiscoveryResult` with found runnable projects sorted by path
pub fn discover_flutter_projects(base_path: &Path, max_depth: usize) -> DiscoveryResult;

/// Check if a directory contains a runnable Flutter project
pub fn is_runnable_flutter_project(path: &Path) -> bool;

/// Get the project type for a directory containing pubspec.yaml
pub fn get_project_type(path: &Path) -> Option<ProjectType>;

/// Check if a project is a Flutter plugin
pub fn is_flutter_plugin(path: &Path) -> bool;

/// Check if project has any platform directories
pub fn has_platform_directories(path: &Path) -> bool;

/// Check if pubspec.yaml has Flutter SDK dependency
pub fn has_flutter_dependency(path: &Path) -> bool;
```

#### Constants

```rust
/// Default maximum search depth
pub const DEFAULT_MAX_DEPTH: usize = 3;

/// Platform directories that indicate a runnable Flutter project
const PLATFORM_DIRECTORIES: &[&str] = &[
    "android",
    "ios",
    "macos",
    "web",
    "linux",
    "windows",
];

/// Subdirectories to check for runnable examples in plugins
const EXAMPLE_DIRECTORIES: &[&str] = &[
    "example",
    "sample",
];

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
    "target",  // Rust build dir
    ".pub-cache",
    ".pub",
];
```

#### Detection Algorithm

```
is_runnable_flutter_project(path):
    1. Check pubspec.yaml exists → if not, return false
    2. Read pubspec.yaml content
    3. Check has_flutter_dependency():
       - Search for "sdk: flutter" in dependencies section
       - If NOT found → return false (Dart-only package)
    4. Check is_flutter_plugin():
       - Search for "plugin:" under "flutter:" section
       - If found → return false (it's a plugin, not directly runnable)
    5. Check has_platform_directories():
       - Check if any of android/, ios/, macos/, web/, linux/, windows/ exist
       - If NONE exist → return false (no runnable target)
    6. Return true

discover_flutter_projects(base_path, max_depth):
    1. Check if base_path is runnable → if yes, return [base_path]
    2. If base_path is a plugin → check example/, sample/ for runnable
    3. Walk directory tree up to max_depth:
       - Skip hidden directories and SKIP_DIRECTORIES
       - For each pubspec.yaml found:
         a. If is_runnable → add to results
         b. If is_plugin → check EXAMPLE_DIRECTORIES for runnable
         c. Don't descend into found projects (avoid nested discovery)
    4. Sort results by path
    5. Return DiscoveryResult
```

#### pubspec.yaml Parsing (Simple String Matching)

To avoid adding a YAML parsing dependency, use simple line-based matching:

```rust
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
        if line.starts_with("flutter:") && !line.starts_with(" ") {
            in_flutter_section = true;
            continue;
        }
        
        // If we hit another root-level key, exit flutter section
        if in_flutter_section && !line.starts_with(" ") && !trimmed.is_empty() {
            in_flutter_section = false;
        }
        
        // Look for plugin: within flutter section
        if in_flutter_section && trimmed.starts_with("plugin:") {
            return true;
        }
    }
    
    false
}

/// Check if pubspec.yaml has flutter SDK dependency
fn check_has_flutter_dependency(content: &str) -> bool {
    // Look for pattern:
    // dependencies:
    //   flutter:
    //     sdk: flutter
    
    // Simple check: look for "sdk: flutter" anywhere in file
    // This is a reasonable heuristic since "sdk: flutter" is unique
    content.contains("sdk: flutter")
}
```

---

### Acceptance Criteria

1. `discover_flutter_projects("/path/to/flutter/app", 3)` returns that path immediately
2. `discover_flutter_projects("/parent/of/flutter/app", 3)` finds the nested app
3. **Flutter plugins are detected** and their `example/` dirs are checked
4. **Dart-only packages** (no `sdk: flutter`) are skipped
5. **Flutter packages without platform dirs** are skipped
6. Hidden directories (`.git`, `.dart_tool`) are not searched
7. Search respects `max_depth` parameter
8. Multiple projects are returned sorted by path
9. Returns empty `Vec` if no runnable projects found (not an error)
10. Handles permission errors gracefully (skips inaccessible dirs)
11. Malformed pubspec.yaml files are handled gracefully

---

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a minimal Flutter app structure
    fn create_flutter_app(path: &Path, name: &str) {
        fs::write(
            path.join("pubspec.yaml"),
            format!(r#"name: {}
dependencies:
  flutter:
    sdk: flutter
flutter:
  uses-material-design: true
"#, name),
        ).unwrap();
        fs::create_dir_all(path.join("lib")).unwrap();
        fs::create_dir_all(path.join("android")).unwrap();
        fs::create_dir_all(path.join("ios")).unwrap();
    }

    /// Helper to create a Flutter plugin structure
    fn create_flutter_plugin(path: &Path, name: &str) {
        fs::write(
            path.join("pubspec.yaml"),
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
        fs::create_dir_all(path.join("lib")).unwrap();
    }

    /// Helper to create a Dart-only package
    fn create_dart_package(path: &Path, name: &str) {
        fs::write(
            path.join("pubspec.yaml"),
            format!(r#"name: {}
dependencies:
  collection: ^1.17.0
"#, name),
        ).unwrap();
        fs::create_dir_all(path.join("lib")).unwrap();
    }

    /// Helper to create a Flutter package (no platform dirs)
    fn create_flutter_package(path: &Path, name: &str) {
        fs::write(
            path.join("pubspec.yaml"),
            format!(r#"name: {}
dependencies:
  flutter:
    sdk: flutter
"#, name),
        ).unwrap();
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
        let names: Vec<_> = result.projects.iter()
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
        assert_eq!(get_project_type(&flutter_pkg), Some(ProjectType::FlutterPackage));
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
}
```

---

### Notes

- Use `std::fs::read_dir` for directory traversal (synchronous is fine for startup)
- **Simple string matching** for pubspec.yaml parsing (no YAML crate needed)
- Log discovered/skipped projects at `debug!` level for troubleshooting
- Log skipped directories at `trace!` level
- Handle malformed pubspec.yaml gracefully (log warning, skip project)
- **Don't descend into found projects** to avoid discovering nested examples twice

---

## Completion Summary

**Status**: ✅ Done

**Files Modified**:
- `src/core/discovery.rs` (created) - Full discovery module implementation
- `src/core/mod.rs` - Added `pub mod discovery` and re-exports
- `Cargo.toml` - Added `tempfile = "3"` dev-dependency for tests

**Notable Decisions/Tradeoffs**:
- Used simple string matching for pubspec.yaml parsing as specified (no YAML crate)
- `check_has_flutter_dependency()` uses `content.contains("sdk: flutter")` heuristic
- `check_is_plugin()` uses line-by-line parsing to detect `plugin:` under `flutter:` section
- Discovery returns immediately if base_path is a runnable project (fast path)
- Plugin examples are discovered by checking EXAMPLE_DIRECTORIES within the plugin
- Skipped projects are tracked in `DiscoveryResult.skipped` for debugging

**Testing Performed**:
- `cargo fmt` - Passed (no formatting issues)
- `cargo check` - Passed (no warnings)
- `cargo test` - All 60 tests passed (17 new discovery tests)

**Test Coverage**:
- `test_is_runnable_flutter_project` - Verifies app detection
- `test_plugin_is_not_runnable` - Verifies plugins are skipped
- `test_dart_package_is_not_runnable` - Verifies Dart packages are skipped
- `test_flutter_package_without_platforms_not_runnable` - Verifies packages without platform dirs skipped
- `test_discover_plugin_example` - Verifies plugin example discovery
- `test_discover_skips_dart_packages` - Verifies Dart packages skipped in search
- `test_discover_multiple_apps` - Verifies multiple apps returned sorted
- `test_has_platform_directories` - Verifies platform dir detection
- `test_get_project_type` - Verifies all project type classifications
- `test_skips_hidden_directories` - Verifies hidden dirs skipped
- `test_respects_max_depth` - Verifies depth limiting
- `test_base_path_is_runnable_app` - Verifies fast path
- `test_discover_plugin_at_base_path` - Verifies plugin at base path
- `test_check_is_plugin` / `test_check_has_flutter_dependency` - Unit tests for parsing
- `test_malformed_pubspec_handled_gracefully` - Verifies error handling
- `test_missing_pubspec_returns_none` - Verifies missing file handling

**Risks/Limitations**:
- Simple string matching may have edge cases with unusual pubspec.yaml formatting
- No symlink loop protection (symlinks are not followed by default with `fs::read_dir`)
- Permission errors on directories are logged at trace level and skipped silently