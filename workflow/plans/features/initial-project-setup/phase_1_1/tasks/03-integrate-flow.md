## Task: Integrate Discovery Flow

**Objective**: Wire the discovery module and project selector into the application entry point (`main.rs`), updating error types to handle plugin/package detection, and ensuring smooth integration with existing Phase 1 functionality.

**Depends on**: [01-discovery-module](01-discovery-module.md), [02-project-selector](02-project-selector.md)

---

### Scope

- `src/main.rs`: Update entry point to use discovery flow
- `src/common/error.rs`: Add new error variants for discovery scenarios
- `src/lib.rs`: Potentially expose new functions if needed

---

### Implementation Details

#### Updated main.rs Flow

```rust
#[tokio::main]
async fn main() -> Result<()> {
    use flutter_demon::core::discovery::{
        discover_flutter_projects, is_runnable_flutter_project, 
        is_flutter_plugin, get_project_type, ProjectType,
        DEFAULT_MAX_DEPTH
    };
    use flutter_demon::tui::selector::{select_project, SelectionResult};

    // Get base path from args or use current directory
    let base_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Step 1: Check if base_path is directly a runnable Flutter project
    if is_runnable_flutter_project(&base_path) {
        return flutter_demon::run_with_project(&base_path).await;
    }

    // Step 2: If base_path has pubspec but isn't runnable, explain why
    if base_path.join("pubspec.yaml").exists() {
        match get_project_type(&base_path) {
            Some(ProjectType::Plugin) => {
                eprintln!("📦 Detected Flutter plugin at: {}", base_path.display());
                eprintln!("   Plugins cannot be run directly. Searching for runnable examples...");
                eprintln!();
            }
            Some(ProjectType::FlutterPackage) => {
                eprintln!("📦 Detected Flutter package at: {}", base_path.display());
                eprintln!("   Package has no platform directories (android/, ios/, etc.).");
                eprintln!("   Searching for runnable projects...");
                eprintln!();
            }
            Some(ProjectType::DartPackage) => {
                eprintln!("📦 Detected Dart package at: {}", base_path.display());
                eprintln!("   Dart-only packages cannot be run with flutter run.");
                eprintln!("   Searching for Flutter projects...");
                eprintln!();
            }
            _ => {}
        }
    }

    // Step 3: Discover runnable Flutter projects in subdirectories
    let discovery = discover_flutter_projects(&base_path, DEFAULT_MAX_DEPTH);

    // Log skipped projects for debugging
    if !discovery.skipped.is_empty() {
        for skipped in &discovery.skipped {
            eprintln!("   Skipped {:?}: {} ({})", 
                skipped.project_type, 
                skipped.path.display(),
                skipped.reason
            );
        }
        eprintln!();
    }

    match discovery.projects.len() {
        0 => {
            // No runnable projects found - show helpful error
            eprintln!("❌ No runnable Flutter projects found in: {}", base_path.display());
            eprintln!("   Searched {} levels deep.", discovery.max_depth);
            eprintln!();
            eprintln!("A runnable Flutter project must have:");
            eprintln!("  • pubspec.yaml with 'sdk: flutter' dependency");
            eprintln!("  • At least one platform directory (android/, ios/, macos/, web/, linux/, windows/)");
            eprintln!("  • NOT be a plugin (no 'flutter: plugin:' section)");
            eprintln!();
            eprintln!("Hint: Run flutter-demon from a Flutter app directory,");
            eprintln!("      or pass the project path as an argument:");
            eprintln!("      flutter-demon /path/to/flutter/app");
            std::process::exit(1);
        }
        1 => {
            // Exactly one runnable project found - auto-select
            let project = &discovery.projects[0];
            eprintln!("✅ Found Flutter project: {}", project.display());
            flutter_demon::run_with_project(project).await
        }
        _ => {
            // Multiple runnable projects found - show selector
            match select_project(&discovery.projects, &discovery.searched_from)? {
                SelectionResult::Selected(project) => {
                    flutter_demon::run_with_project(&project).await
                }
                SelectionResult::Cancelled => {
                    eprintln!("Selection cancelled.");
                    Ok(())
                }
            }
        }
    }
}
```

#### New Error Variants

Add to `src/common/error.rs`:

```rust
// ─────────────────────────────────────────────────────────────
// Discovery Errors
// ─────────────────────────────────────────────────────────────

#[error("No runnable Flutter projects found in: {searched_path}")]
NoRunnableProjects { searched_path: PathBuf },

#[error("Project selection was cancelled by user")]
SelectionCancelled,

#[error("Discovery error: {message}")]
Discovery { message: String },

#[error("Directory is a Flutter plugin, not a runnable app: {path}")]
IsPlugin { path: PathBuf },

#[error("Directory is a Dart package, not a Flutter app: {path}")]
IsDartPackage { path: PathBuf },

#[error("Flutter package has no platform directories: {path}")]
NoPlatformDirectories { path: PathBuf },
```

Add convenience constructors:

```rust
impl Error {
    // ... existing constructors ...

    pub fn no_runnable_projects(path: impl Into<PathBuf>) -> Self {
        Self::NoRunnableProjects {
            searched_path: path.into(),
        }
    }

    pub fn discovery(message: impl Into<String>) -> Self {
        Self::Discovery {
            message: message.into(),
        }
    }

    pub fn is_plugin(path: impl Into<PathBuf>) -> Self {
        Self::IsPlugin { path: path.into() }
    }

    pub fn is_dart_package(path: impl Into<PathBuf>) -> Self {
        Self::IsDartPackage { path: path.into() }
    }

    pub fn no_platform_directories(path: impl Into<PathBuf>) -> Self {
        Self::NoPlatformDirectories { path: path.into() }
    }
}
```

Update `is_fatal()` to include new error types:

```rust
pub fn is_fatal(&self) -> bool {
    matches!(
        self,
        Error::FlutterNotFound
            | Error::NoProject { .. }
            | Error::NoRunnableProjects { .. }
            | Error::ProcessSpawn { .. }
            | Error::TerminalInit(_)
    )
}
```

Update `is_recoverable()` for discovery errors:

```rust
pub fn is_recoverable(&self) -> bool {
    matches!(
        self,
        Error::Daemon { .. } 
            | Error::Protocol { .. } 
            | Error::ChannelSend { .. }
            | Error::SelectionCancelled  // User chose to cancel
    )
}
```

---

### Acceptance Criteria

1. Running `flutter-demon` from a Flutter project dir works as before (no change)
2. Running `flutter-demon` from parent dir discovers and uses child project
3. Running `flutter-demon` from dir with multiple Flutter subdirs shows selector
4. Running `flutter-demon` from dir with no Flutter projects shows helpful error
5. Running `flutter-demon /path/to/project` with explicit path works as before
6. Cancelled selection exits cleanly with exit code 0
7. No projects found exits with exit code 1
8. Error types are properly categorized (fatal/recoverable)
9. Logging captures discovery actions for troubleshooting

---

### Integration Points

#### With Discovery Module (Task 01)

```rust
use flutter_demon::core::discovery::{
    discover_flutter_projects,
    DiscoveryResult,
    DEFAULT_MAX_DEPTH,
};

let result: DiscoveryResult = discover_flutter_projects(&base_path, DEFAULT_MAX_DEPTH);
// result.projects: Vec<PathBuf>
// result.searched_from: PathBuf
// result.max_depth: usize
```

#### With Project Selector (Task 02)

```rust
use flutter_demon::tui::selector::{select_project, SelectionResult};

match select_project(&projects, &base_path)? {
    SelectionResult::Selected(path) => { /* use path */ }
    SelectionResult::Cancelled => { /* handle cancel */ }
}
```

#### With Existing run_with_project

```rust
// Existing function - no changes needed
pub async fn run_with_project(project_path: &Path) -> Result<()>;
```

---

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Note: Main function testing is typically done via integration tests

    #[test]
    fn test_error_no_projects_found() {
        let err = Error::no_projects_found("/test/path");
        assert!(err.to_string().contains("/test/path"));
        assert!(err.is_fatal());
    }

    #[test]
    fn test_error_selection_cancelled() {
        let err = Error::SelectionCancelled;
        assert!(!err.is_fatal()); // Not fatal, just user choice
    }

    #[test]
    fn test_error_discovery() {
        let err = Error::discovery("permission denied");
        assert!(err.to_string().contains("permission denied"));
    }
}
```

---

### CLI Behavior Matrix

| Scenario | Command | Result |
|----------|---------|--------|
| Direct Flutter app | `cd my_app && flutter-demon` | Starts TUI immediately |
| Parent with one child | `cd parent && flutter-demon` | Auto-selects, starts TUI |
| Parent with multiple | `cd parent && flutter-demon` | Shows selector |
| Explicit path | `flutter-demon /path/to/app` | Starts TUI with that path |
| No runnable projects | `cd empty && flutter-demon` | Error message, exit 1 |
| User cancels | (presses 'q' in selector) | Clean exit, exit 0 |
| **In a Flutter plugin** | `cd my_plugin && flutter-demon` | Finds `example/`, starts TUI |
| **Plugin with no example** | `cd plugin_no_example && flutter-demon` | Error: no runnable projects |
| **In a Dart package** | `cd dart_utils && flutter-demon` | Error: Dart package not runnable |
| **Flutter package (no platforms)** | `cd flutter_utils && flutter-demon` | Error: no platform directories |
| **Monorepo with mixed** | `cd monorepo && flutter-demon` | Shows only runnable apps in selector |

---

### Logging Strategy

Add logging at key decision points:

```rust
// On startup
debug!("Starting flutter-demon, base path: {}", base_path.display());

// On direct project detection
info!("Flutter project detected at base path");

// On discovery
info!("Searching for Flutter projects...");
debug!("Discovered {} project(s)", discovery.projects.len());

// On auto-selection
info!("Auto-selecting single discovered project: {}", project.display());

// On selection
info!("User selected project: {}", selected.display());

// On cancellation
info!("User cancelled project selection");

// On no projects
warn!("No Flutter projects found in: {}", base_path.display());
```

---

### Notes

- Keep `main.rs` thin - discovery logic lives in modules
- Exit codes: 0 for success/cancel, 1 for errors
- The flow should feel instant for the common case (project at base path)
- **Provide clear feedback** when a plugin/package is detected at base path
- **Show skipped projects** in verbose mode for debugging
- Consider adding `--list` or `--discover` flag for explicit discovery mode (future)
- Consider adding `--verbose` flag to show all skipped projects

### User-Friendly Messages

The error messages should be helpful and guide users:

```
📦 Detected Flutter plugin at: /path/to/my_plugin
   Plugins cannot be run directly. Searching for runnable examples...

✅ Found Flutter project: /path/to/my_plugin/example
```

```
❌ No runnable Flutter projects found in: /path/to/workspace
   Searched 3 levels deep.

A runnable Flutter project must have:
  • pubspec.yaml with 'sdk: flutter' dependency
  • At least one platform directory (android/, ios/, macos/, web/, linux/, windows/)
  • NOT be a plugin (no 'flutter: plugin:' section)

Hint: Run flutter-demon from a Flutter app directory,
      or pass the project path as an argument:
      flutter-demon /path/to/flutter/app
```

---

## Completion Summary

**Status**: ✅ Done

**Files Modified**:
- `src/main.rs` - Complete rewrite with discovery flow integration
- `src/common/error.rs` - Added 6 new discovery-related error variants

**New Error Variants**:
- `NoRunnableProjects { searched_path }` - No runnable projects found
- `SelectionCancelled` - User cancelled project selection
- `Discovery { message }` - Generic discovery error
- `IsPlugin { path }` - Directory is a Flutter plugin
- `IsDartPackage { path }` - Directory is a Dart package
- `NoPlatformDirectories { path }` - Flutter package without platform dirs

**main.rs Flow**:
1. Check if base_path is directly a runnable Flutter project → run immediately
2. If pubspec.yaml exists but not runnable, explain why (plugin/package/dart)
3. Discover runnable Flutter projects in subdirectories
4. Handle results:
   - 0 projects: Show helpful error message, exit code 1
   - 1 project: Auto-select, show confirmation, run TUI
   - Multiple projects: Show interactive selector, run selected

**User Experience**:
- Clear emoji-prefixed messages (📦, ✅, ❌)
- Explains why non-runnable projects are skipped
- Shows skipped projects when no runnable projects found
- Provides actionable hints for resolution
- Correct exit codes (0 for success/cancel, 1 for errors)

**Testing Performed**:
- `cargo fmt` - Passed (no formatting issues)
- `cargo check` - No warnings
- `cargo test` - All 75 tests passed (7 new error tests)

**New Tests**:
- `test_discovery_error_constructors` - All new error constructors work
- `test_no_runnable_projects_error` - Error message and is_fatal()
- `test_selection_cancelled_error` - Not fatal, is recoverable
- `test_discovery_error` - Generic discovery error
- `test_is_plugin_error` - Plugin error message
- `test_is_dart_package_error` - Dart package error message
- `test_no_platform_directories_error` - No platform dirs error

**Risks/Limitations**:
- No `--verbose` flag yet (skipped projects only shown when no results)
- No `--list` or `--discover` flag for explicit discovery mode
- Logging with tracing not added (uses eprintln for now)