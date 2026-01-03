# Phase 1.1: Flutter Project Discovery - Task Index

## Overview

**Goal**: Automatically discover **runnable** Flutter projects when running `flutter-demon` from a parent directory, filtering out plugins/packages without runnable targets, and allow users to select from multiple discovered projects.

**Duration**: 0.5-1 week

**Total Tasks**: 4

This subphase addresses a usability gap discovered after Phase 1 completion. When users run `flutter-demon` from a directory that doesn't directly contain a Flutter project (but has Flutter projects in subdirectories), the app should intelligently search for and offer project selection.

### Problem Statement

Currently, running `flutter-demon` from a parent directory (e.g., `/Users/ed/Dev/zabin/flutter-demon`) fails with:
```
Failed to start Flutter: No Flutter project found in: /Users/ed/Dev/zabin/flutter-demon
```

Even though a valid Flutter project exists at `./sample/`.

Additionally, users working on **Flutter plugins** may have a `pubspec.yaml` at the root, but it's not a runnable target—the runnable example is typically in `example/` or `sample/` subdirectories.

### Desired Behavior

1. **Priority 1**: Check if PWD contains a **runnable** Flutter project
2. **Priority 2**: If not runnable (or is a plugin), search for runnable targets in subdirectories
3. **Smart Filtering**: Only show projects that can actually be run with `flutter run`
4. **Plugin Support**: For plugins, automatically discover runnable examples in `example/` or `sample/`
5. **Single Project**: Auto-select if exactly one runnable project is discovered
6. **Multiple Projects**: Present an interactive selection menu
7. **No Projects**: Show a helpful error message with search location

### Project Type Classification

| Type | Runnable? | Characteristics |
|------|-----------|-----------------|
| **Flutter Application** | ✅ Yes | Has `flutter: sdk` dependency, has platform dirs (android/, ios/, etc.), NO `flutter: plugin:` section |
| **Flutter Plugin** | ❌ No | Has `flutter: plugin: platforms:` section in pubspec.yaml |
| **Flutter Package** | ❌ No | Has `flutter: sdk` dependency but no platform directories |
| **Dart Package** | ❌ No | No `flutter: sdk` dependency, pure Dart code |

For **plugins**, we recursively check `example/` and `sample/` subdirectories for runnable targets.

---

## Task Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   ┌──────────────────────┐                                                  │
│   │  01-discovery-module │                                                  │
│   │  (search for Flutter │                                                  │
│   │   projects in dirs)  │                                                  │
│   └────────┬─────────────┘                                                  │
│            │                                                                │
│            ▼                                                                │
│   ┌──────────────────────┐                                                  │
│   │  02-project-selector │                                                  │
│   │  (interactive menu   │                                                  │
│   │   for multi-project) │                                                  │
│   └────────┬─────────────┘                                                  │
│            │                                                                │
│            ▼                                                                │
│   ┌──────────────────────┐                                                  │
│   │  03-integrate-flow   │                                                  │
│   │  (update main.rs and │                                                  │
│   │   app entry points)  │                                                  │
│   └────────┬─────────────┘                                                  │
│            │                                                                │
│            ▼                                                                │
│   ┌──────────────────────┐                                                  │
│   │  04-testing-docs     │                                                  │
│   │  (test scenarios,    │                                                  │
│   │   update docs)       │                                                  │
│   └──────────────────────┘                                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Tasks

| # | Task | Status | Depends On | Effort | Key Modules |
|---|------|--------|------------|--------|-------------|
| 1 | [01-discovery-module](tasks/01-discovery-module.md) | Not Started | - | 2-3 hrs | `core/discovery.rs` |
| 2 | [02-project-selector](tasks/02-project-selector.md) | Not Started | 01 | 2-3 hrs | `tui/selector.rs` |
| 3 | [03-integrate-flow](tasks/03-integrate-flow.md) | Not Started | 01, 02 | 1-2 hrs | `main.rs`, `common/error.rs` |
| 4 | [04-testing-docs](tasks/04-testing-docs.md) | Not Started | 03 | 1 hr | tests, README |

**Total Estimated Effort**: 6-9 hours

### Task Summaries

| Task | Description |
|------|-------------|
| **01-discovery-module** | Create `core/discovery.rs` with recursive project search, depth limiting, and directory filtering |
| **02-project-selector** | Create interactive terminal selector for choosing between multiple discovered projects |
| **03-integrate-flow** | Wire discovery and selector into `main.rs` app entry point, update error types |
| **04-testing-docs** | Add integration tests for discovery scenarios, update documentation |

---

## Technical Design

### Project Type Detection

To determine if a project is runnable, we need to check:

1. **Parse `pubspec.yaml`**:
   - Check for `dependencies.flutter.sdk: flutter` → Has Flutter dependency
   - Check for `flutter.plugin.platforms` section → Is a plugin (NOT directly runnable)

2. **Check platform directories**:
   - `android/` - Android support
   - `ios/` - iOS support
   - `macos/` - macOS support
   - `web/` - Web support
   - `linux/` - Linux support
   - `windows/` - Windows support
   
   At least ONE must exist for the project to be runnable.

### Detection Functions

```rust
/// Project type classification
pub enum ProjectType {
    /// A runnable Flutter application
    Application,
    /// A Flutter plugin (check example/ for runnable target)
    Plugin,
    /// A Flutter package (has flutter dep but no platform dirs)
    FlutterPackage,
    /// A pure Dart package (not runnable with flutter run)
    DartPackage,
}

/// Check if a directory is a runnable Flutter project
pub fn is_runnable_flutter_project(path: &Path) -> bool {
    // 1. pubspec.yaml must exist
    // 2. Must have flutter SDK dependency
    // 3. Must NOT be a plugin
    // 4. Must have at least one platform directory
}

/// Check if a project is a Flutter plugin
pub fn is_flutter_plugin(path: &Path) -> bool {
    // Check for `flutter: plugin: platforms:` in pubspec.yaml
}

/// Check if project has any platform directories
pub fn has_platform_directories(path: &Path) -> bool {
    // Check for android/, ios/, macos/, web/, linux/, windows/
}

/// Get the project type for a directory
pub fn get_project_type(path: &Path) -> Option<ProjectType>
```

### pubspec.yaml Parsing

We need to check specific fields in pubspec.yaml. Options:

1. **Simple string matching** (minimal dependencies):
   - Search for `sdk: flutter` in dependencies section
   - Search for `plugin:` under `flutter:` section

2. **YAML parsing** (more robust, add `serde_yaml` crate):
   - Parse full structure
   - Navigate to specific fields

**Recommendation**: Use simple string/line matching for Phase 1.1 to avoid adding dependencies. Can upgrade to full YAML parsing later if needed.

### Discovery Algorithm

```
discover_runnable_projects(base_path, max_depth=3):
    1. Check if base_path is a runnable project
       - If YES: return [base_path]
       - If it's a PLUGIN: check example/, sample/ subdirs for runnable targets
    
    2. Initialize results = []
    
    3. Walk directory tree up to max_depth:
       - Skip hidden directories (starting with '.')
       - Skip common excluded: node_modules, build, .dart_tool, etc.
       - If directory contains pubspec.yaml:
         a. If is_runnable_flutter_project() → add to results
         b. Else if is_flutter_plugin() → check example/, sample/ for runnable
         c. Skip this directory's subdirs (don't nest into found projects)
    
    4. Return results sorted by path

is_runnable_flutter_project(path):
    1. pubspec.yaml must exist
    2. Parse pubspec.yaml:
       - Must contain `sdk: flutter` in dependencies
       - Must NOT contain `plugin:` section under `flutter:`
    3. Must have at least one platform dir: android/, ios/, macos/, web/, linux/, windows/
    4. Return true if all conditions met
```

### Selector UX Flow

```
┌──────────────────────────────────────────────────────────────┐
│  Multiple Flutter projects found. Select one:                │
│                                                              │
│  [1] sample                                                  │
│  [2] my_plugin/example                                       │
│  [3] examples/counter_app                                    │
│  [4] examples/todo_app                                       │
│                                                              │
│  Enter number (1-4) or 'q' to quit: _                        │
└──────────────────────────────────────────────────────────────┘
```

Note: Plugin examples are shown with their full relative path (e.g., `my_plugin/example`).

### Module Location Rationale

| Module | Location | Reason |
|--------|----------|--------|
| `discovery.rs` | `core/` | Core capability, no TUI/daemon dependencies |
| `selector.rs` | `tui/` | Interactive terminal UI component |

### Platform Directory Constants

```rust
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
```

---

## Success Criteria

Phase 1.1 is complete when:

- [ ] Running `flutter-demon` from a parent directory auto-discovers **runnable** Flutter projects
- [ ] Single discovered project is auto-selected without prompts
- [ ] Multiple discovered projects show numbered selection menu
- [ ] User can select project by pressing number key (1-9)
- [ ] User can cancel selection with 'q' or Ctrl+C (clean exit)
- [ ] No projects found shows helpful error with searched path
- [ ] Search respects max depth (default 3 levels) to avoid slow searches
- [ ] Hidden directories (`.git`, `.dart_tool`, etc.) are skipped
- [ ] Common build/dependency directories are skipped (`build/`, `node_modules/`)
- [ ] **Flutter plugins are detected and their `example/` dirs are checked for runnable targets**
- [ ] **Dart-only packages (no flutter dependency) are skipped**
- [ ] **Flutter packages without platform directories are skipped**
- [ ] **Projects must have at least one platform dir (android/, ios/, etc.) to be considered runnable**
- [ ] All existing Phase 1 functionality still works when run from project dir
- [ ] Unit tests cover discovery edge cases including plugin detection
- [ ] `cargo test` passes with new tests

---

## Edge Cases & Risks

| Risk | Mitigation |
|------|------------|
| Slow search in large directories | Max depth limit (default 3), skip known large dirs |
| Symlink loops | Don't follow symlinks in search |
| Permission errors on subdirs | Gracefully skip inaccessible directories, log warning |
| User expects different project | Show relative paths clearly in selector |
| Multiple pubspec.yaml in nested dirs | Return topmost project, skip subdirs of found projects |
| Plugin at root with no example/ | Show helpful message: "Plugin detected but no runnable example found" |
| Malformed pubspec.yaml | Gracefully handle parse errors, skip project with warning |
| Project with pubspec but deleted platform dirs | Detected as non-runnable (correct behavior) |
| Monorepo with many packages | Only show runnable apps, not internal packages |
| Federated plugin structure | Check each platform package's example/ |

---

## Notes

- This is a "quality of life" enhancement discovered during Phase 1 testing
- Keeps the simple case simple (project at PWD works exactly as before)
- Selector UI is intentionally simple (no full TUI, just terminal I/O)
- Could be extended later to remember last selection per parent directory
- **Plugin detection uses simple string matching** to avoid adding YAML parsing dependencies
- **Platform directory check is fast** (just checking if directories exist)

## pubspec.yaml Detection Patterns

### Flutter Application (Runnable ✅)
```yaml
dependencies:
  flutter:
    sdk: flutter

flutter:
  uses-material-design: true
```
+ Has `android/`, `ios/`, or other platform directories

### Flutter Plugin (Not Runnable ❌, check example/)
```yaml
dependencies:
  flutter:
    sdk: flutter

flutter:
  plugin:
    platforms:
      android:
        package: com.example.my_plugin
        pluginClass: MyPlugin
      ios:
        pluginClass: MyPlugin
```

### Dart Package (Not Runnable ❌)
```yaml
dependencies:
  # No flutter SDK dependency
  collection: ^1.17.0
```

### Flutter Package (Not Runnable ❌)
```yaml
dependencies:
  flutter:
    sdk: flutter

flutter:
  # No plugin section, but also no platform directories
```