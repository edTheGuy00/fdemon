## Task: Create discover_entry_points() function

**Objective**: Implement a function to discover all Dart files containing `main()` functions in a Flutter project's `lib/` directory.

**Depends on**: Task 01 (has_main_function)

### Scope

- `src/core/discovery.rs`: Add `discover_entry_points()` function

### Details

Create a function that scans the `lib/` directory of a Flutter project and returns all Dart files containing a `main()` function, suitable for use as entry points with `flutter run -t`.

#### Requirements

1. Scan only the `lib/` directory (not test/, build/, etc.)
2. Recursively traverse subdirectories
3. Check each `.dart` file for `main()` function
4. Return paths relative to project root (e.g., `lib/main.dart`)
5. Sort results with `main.dart` first, then alphabetically
6. Handle missing `lib/` directory gracefully (return empty vec)

#### Implementation

Add to `src/core/discovery.rs`:

```rust
/// Discovers Dart files containing a main() function in the lib/ directory.
///
/// This function scans the `lib/` directory of a Flutter project and identifies
/// files that can be used as entry points with `flutter run -t <path>`.
///
/// # Arguments
///
/// * `project_path` - Path to the Flutter project root (containing pubspec.yaml)
///
/// # Returns
///
/// A vector of paths relative to project root, sorted with:
/// 1. `lib/main.dart` first (if exists)
/// 2. Other files with `main.dart` filename
/// 3. Remaining files alphabetically
///
/// Returns an empty vector if:
/// - The `lib/` directory doesn't exist
/// - No Dart files contain a main() function
/// - Any I/O errors occur
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use flutter_demon::core::discovery::discover_entry_points;
///
/// let project = Path::new("/path/to/flutter/app");
/// let entry_points = discover_entry_points(project);
///
/// // Might return:
/// // [
/// //   "lib/main.dart",
/// //   "lib/main_dev.dart",
/// //   "lib/main_staging.dart",
/// //   "lib/flavors/main_prod.dart",
/// // ]
/// ```
pub fn discover_entry_points(project_path: &Path) -> Vec<PathBuf> {
    let lib_path = project_path.join("lib");

    if !lib_path.is_dir() {
        trace!("No lib/ directory found at {:?}", lib_path);
        return Vec::new();
    }

    let mut entry_points = Vec::new();
    discover_entry_points_recursive(&lib_path, project_path, &mut entry_points);

    // Sort with main.dart files first, then alphabetically
    entry_points.sort_by(|a, b| {
        let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let a_is_main = a_name == "main.dart";
        let b_is_main = b_name == "main.dart";

        // Primary sort: main.dart files first
        match (a_is_main, b_is_main) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                // Secondary sort: lib/main.dart before nested main.dart
                let a_is_lib_main = a.as_os_str() == "lib/main.dart"
                    || a.as_os_str() == "lib\\main.dart";
                let b_is_lib_main = b.as_os_str() == "lib/main.dart"
                    || b.as_os_str() == "lib\\main.dart";

                match (a_is_lib_main, b_is_lib_main) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.cmp(b), // Alphabetical
                }
            }
        }
    });

    debug!(
        "Discovered {} entry points in {:?}",
        entry_points.len(),
        project_path
    );
    entry_points
}

/// Recursive helper for entry point discovery.
fn discover_entry_points_recursive(
    dir: &Path,
    project_root: &Path,
    results: &mut Vec<PathBuf>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            trace!("Cannot read directory {:?}: {}", dir, err);
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            // Skip hidden directories
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }
            // Recurse into subdirectory
            discover_entry_points_recursive(&path, project_root, results);
        } else if path.extension().map_or(false, |ext| ext == "dart") {
            // Check if this Dart file has a main() function
            if has_main_function(&path) {
                if let Ok(relative) = path.strip_prefix(project_root) {
                    trace!("Found entry point: {:?}", relative);
                    results.push(relative.to_path_buf());
                }
            }
        }
    }
}
```

### Acceptance Criteria

1. `discover_entry_points()` returns empty vec when `lib/` doesn't exist
2. `discover_entry_points()` finds `lib/main.dart` when it contains `main()`
3. `discover_entry_points()` finds files in nested directories (e.g., `lib/flavors/main_dev.dart`)
4. `discover_entry_points()` ignores `.dart` files without `main()` function
5. `discover_entry_points()` returns paths relative to project root
6. Results are sorted with `lib/main.dart` first
7. Other `main.dart` files come before non-main.dart files
8. Remaining files sorted alphabetically
9. Hidden directories (starting with `.`) are skipped
10. Code compiles without errors

### Testing

Add these tests to the `mod tests` block in `src/core/discovery.rs`:

```rust
/// Helper to create a Dart file with content
fn write_dart_file(base: &Path, relative_path: &str, content: &str) {
    let full_path = base.join(relative_path);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(full_path, content).unwrap();
}

#[test]
fn test_discover_entry_points_basic() {
    let temp = TempDir::new().unwrap();

    write_dart_file(temp.path(), "lib/main.dart", "void main() {}");
    write_dart_file(temp.path(), "lib/utils.dart", "void helper() {}");

    let entry_points = discover_entry_points(temp.path());

    assert_eq!(entry_points.len(), 1);
    assert_eq!(entry_points[0], PathBuf::from("lib/main.dart"));
}

#[test]
fn test_discover_entry_points_multiple() {
    let temp = TempDir::new().unwrap();

    write_dart_file(temp.path(), "lib/main.dart", "void main() {}");
    write_dart_file(temp.path(), "lib/main_dev.dart", "void main() {}");
    write_dart_file(temp.path(), "lib/main_staging.dart", "void main() {}");
    write_dart_file(temp.path(), "lib/utils.dart", "void helper() {}");

    let entry_points = discover_entry_points(temp.path());

    assert_eq!(entry_points.len(), 3);
    // main.dart should be first
    assert_eq!(entry_points[0], PathBuf::from("lib/main.dart"));
    // Others alphabetically
    assert!(entry_points.contains(&PathBuf::from("lib/main_dev.dart")));
    assert!(entry_points.contains(&PathBuf::from("lib/main_staging.dart")));
}

#[test]
fn test_discover_entry_points_nested_directories() {
    let temp = TempDir::new().unwrap();

    write_dart_file(temp.path(), "lib/main.dart", "void main() {}");
    write_dart_file(temp.path(), "lib/flavors/dev/main.dart", "void main() {}");
    write_dart_file(temp.path(), "lib/flavors/main_prod.dart", "void main() {}");

    let entry_points = discover_entry_points(temp.path());

    assert_eq!(entry_points.len(), 3);
    // lib/main.dart first
    assert_eq!(entry_points[0], PathBuf::from("lib/main.dart"));
    // Nested main.dart second
    assert_eq!(entry_points[1], PathBuf::from("lib/flavors/dev/main.dart"));
}

#[test]
fn test_discover_entry_points_no_lib_directory() {
    let temp = TempDir::new().unwrap();
    // Don't create lib/ directory

    let entry_points = discover_entry_points(temp.path());

    assert!(entry_points.is_empty());
}

#[test]
fn test_discover_entry_points_empty_lib() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("lib")).unwrap();

    let entry_points = discover_entry_points(temp.path());

    assert!(entry_points.is_empty());
}

#[test]
fn test_discover_entry_points_no_main_functions() {
    let temp = TempDir::new().unwrap();

    write_dart_file(temp.path(), "lib/widget.dart", "class MyWidget {}");
    write_dart_file(temp.path(), "lib/utils.dart", "void helper() {}");

    let entry_points = discover_entry_points(temp.path());

    assert!(entry_points.is_empty());
}

#[test]
fn test_discover_entry_points_skips_hidden_directories() {
    let temp = TempDir::new().unwrap();

    write_dart_file(temp.path(), "lib/main.dart", "void main() {}");
    write_dart_file(temp.path(), "lib/.hidden/secret_main.dart", "void main() {}");

    let entry_points = discover_entry_points(temp.path());

    assert_eq!(entry_points.len(), 1);
    assert_eq!(entry_points[0], PathBuf::from("lib/main.dart"));
}

#[test]
fn test_discover_entry_points_only_scans_lib() {
    let temp = TempDir::new().unwrap();

    write_dart_file(temp.path(), "lib/main.dart", "void main() {}");
    write_dart_file(temp.path(), "test/main.dart", "void main() {}");
    write_dart_file(temp.path(), "bin/main.dart", "void main() {}");

    let entry_points = discover_entry_points(temp.path());

    assert_eq!(entry_points.len(), 1);
    assert_eq!(entry_points[0], PathBuf::from("lib/main.dart"));
}

#[test]
fn test_discover_entry_points_sorting() {
    let temp = TempDir::new().unwrap();

    write_dart_file(temp.path(), "lib/zebra_main.dart", "void main() {}");
    write_dart_file(temp.path(), "lib/alpha_main.dart", "void main() {}");
    write_dart_file(temp.path(), "lib/main.dart", "void main() {}");
    write_dart_file(temp.path(), "lib/sub/main.dart", "void main() {}");

    let entry_points = discover_entry_points(temp.path());

    assert_eq!(entry_points.len(), 4);
    // lib/main.dart first
    assert_eq!(entry_points[0], PathBuf::from("lib/main.dart"));
    // Nested main.dart second
    assert_eq!(entry_points[1], PathBuf::from("lib/sub/main.dart"));
    // Then alphabetically
    assert_eq!(entry_points[2], PathBuf::from("lib/alpha_main.dart"));
    assert_eq!(entry_points[3], PathBuf::from("lib/zebra_main.dart"));
}

#[test]
fn test_discover_entry_points_async_main() {
    let temp = TempDir::new().unwrap();

    write_dart_file(
        temp.path(),
        "lib/main.dart",
        "Future<void> main() async { await init(); }",
    );

    let entry_points = discover_entry_points(temp.path());

    assert_eq!(entry_points.len(), 1);
    assert_eq!(entry_points[0], PathBuf::from("lib/main.dart"));
}
```

### Notes

- Follows existing patterns in `discovery.rs` for directory traversal
- Uses `tracing::trace!` and `tracing::debug!` for logging (consistent with module)
- Does NOT use `walkdir` crate - uses `std::fs::read_dir` like existing code
- Performance is acceptable for typical Flutter projects (< 1000 files in lib/)
- Paths use forward slashes on all platforms for consistency with Flutter conventions

### Performance Considerations

For very large projects, this function reads every `.dart` file in `lib/`. Mitigations:

1. Only scans `lib/` (not entire project)
2. Short-circuits file reading on first match of main() regex
3. Results could be cached per session (Phase 3 responsibility)

Typical Flutter app has < 100 files in lib/, so performance should not be an issue.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/core/discovery.rs` | Added `discover_entry_points()` and `discover_entry_points_recursive()` functions with full documentation and 10 comprehensive unit tests |

### Notable Decisions/Tradeoffs

1. **Used `is_some_and()` instead of `map_or()`**: Clippy flagged the use of `map_or(false, |ext| ext == "dart")` and suggested `is_some_and()` which is cleaner and more idiomatic.
2. **Path sorting logic**: Implemented two-level sorting where `lib/main.dart` comes first, then other `main.dart` files (e.g., nested ones), then alphabetically. This matches Flutter conventions where the default entry point is always `lib/main.dart`.
3. **Hidden directory filtering**: Skips directories starting with `.` to avoid scanning `.dart_tool`, `.git`, etc., improving performance and avoiding false positives.
4. **Relative paths**: Returns paths relative to project root (e.g., `lib/main.dart`) which is exactly what `flutter run -t` expects.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib discovery::tests::test_discover_entry_points` - Passed (10/10 tests)
- `cargo clippy -- -D warnings` - Passed

All 10 test cases cover the acceptance criteria:
1. Empty lib directory handling
2. Finding basic entry points
3. Multiple entry points
4. Nested directories
5. Files without main() functions
6. Hidden directory filtering
7. Lib-only scanning (ignores test/, bin/)
8. Sorting with lib/main.dart first
9. Async main() support

### Risks/Limitations

1. **Performance on large projects**: For projects with thousands of Dart files in `lib/`, this may be slow as it reads each file. Mitigated by scanning only `lib/` directory and using efficient regex matching. Caching can be added in Phase 3 if needed.
2. **Regex false positives**: The main() detection regex may match commented-out code in multi-line comments, but this is acceptable as false positives are better than missing valid entry points (users can manually specify paths if needed).
