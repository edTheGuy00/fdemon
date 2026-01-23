## Task: Add main() function detection helpers

**Objective**: Create helper functions to detect whether a Dart file contains a `main()` function entry point.

**Depends on**: None

### Scope

- `src/core/discovery.rs`: Add `has_main_function()` and `has_main_function_in_content()` helper functions

### Details

Create two helper functions for detecting main() functions in Dart files:

1. **`has_main_function_in_content(content: &str) -> bool`** - Checks if string content contains a main() function
2. **`has_main_function(path: &Path) -> bool`** - Reads a file and checks for main() function

#### Main function patterns to detect

Dart main functions can have several forms:

```dart
// Standard forms
void main() { }
void main(List<String> args) { }
main() { }
main(List<String> args) { }

// Async forms
Future<void> main() async { }
Future<void> main(List<String> args) async { }

// With annotations
@pragma('vm:entry-point')
void main() { }
```

#### Implementation

Add to `src/core/discovery.rs`:

```rust
use regex::Regex;
use std::sync::LazyLock;

/// Regex patterns for detecting Dart main() function declarations.
///
/// Matches:
/// - `void main(` - standard void return
/// - `main(` - implicit dynamic return
/// - `Future<void> main(` - async main
/// - `FutureOr<void> main(` - sync or async main
///
/// Does NOT match:
/// - `// void main(` - commented out
/// - `notmain(` - different function name
/// - `_main(` - private function
static MAIN_FUNCTION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^[^/\n]*\b(?:void|Future<void>|FutureOr<void>)?\s*main\s*\("
    ).expect("Invalid main function regex")
});

/// Check if Dart file content contains a main() function declaration.
///
/// This uses a regex-based heuristic that handles common patterns:
/// - `void main()`, `main()`, `Future<void> main() async`
/// - Ignores single-line comments (`//`)
///
/// Note: This may have false positives for main() in multi-line comments
/// or strings, but these edge cases are acceptable since:
/// 1. Users can always type a custom path in the UI
/// 2. False positives are better than missing valid entry points
///
/// # Examples
///
/// ```
/// use flutter_demon::core::discovery::has_main_function_in_content;
///
/// assert!(has_main_function_in_content("void main() {}"));
/// assert!(has_main_function_in_content("Future<void> main() async {}"));
/// assert!(!has_main_function_in_content("void notMain() {}"));
/// assert!(!has_main_function_in_content("// void main() {}"));
/// ```
pub fn has_main_function_in_content(content: &str) -> bool {
    MAIN_FUNCTION_REGEX.is_match(content)
}

/// Check if a Dart file at the given path contains a main() function.
///
/// Returns `false` if the file cannot be read or doesn't contain main().
///
/// # Arguments
///
/// * `path` - Path to the Dart file to check
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use flutter_demon::core::discovery::has_main_function;
///
/// let has_main = has_main_function(Path::new("lib/main.dart"));
/// ```
pub fn has_main_function(path: &Path) -> bool {
    match fs::read_to_string(path) {
        Ok(content) => has_main_function_in_content(&content),
        Err(_) => false,
    }
}
```

### Acceptance Criteria

1. `has_main_function_in_content()` returns `true` for standard `void main()` declarations
2. `has_main_function_in_content()` returns `true` for `main()` without return type
3. `has_main_function_in_content()` returns `true` for `Future<void> main() async`
4. `has_main_function_in_content()` returns `false` for functions named `notMain` or `_main`
5. `has_main_function_in_content()` returns `false` for single-line commented main
6. `has_main_function()` reads file and delegates to content checker
7. `has_main_function()` returns `false` for non-existent files
8. Code compiles without errors
9. Uses `LazyLock` for regex compilation (compile once)

### Testing

Add these tests to the `mod tests` block in `src/core/discovery.rs`:

```rust
#[test]
fn test_has_main_function_void_main() {
    assert!(has_main_function_in_content("void main() {}"));
    assert!(has_main_function_in_content("void main(List<String> args) {}"));
    assert!(has_main_function_in_content("void main() {\n  runApp(MyApp());\n}"));
}

#[test]
fn test_has_main_function_implicit_return() {
    assert!(has_main_function_in_content("main() {}"));
    assert!(has_main_function_in_content("main(List<String> args) {}"));
}

#[test]
fn test_has_main_function_async() {
    assert!(has_main_function_in_content("Future<void> main() async {}"));
    assert!(has_main_function_in_content("Future<void> main(List<String> args) async {}"));
}

#[test]
fn test_has_main_function_with_whitespace() {
    assert!(has_main_function_in_content("void  main() {}"));
    assert!(has_main_function_in_content("void main () {}"));
    assert!(has_main_function_in_content("  void main() {}"));
    assert!(has_main_function_in_content("\nvoid main() {}"));
}

#[test]
fn test_has_main_function_rejects_non_main() {
    assert!(!has_main_function_in_content("void notMain() {}"));
    assert!(!has_main_function_in_content("void _main() {}"));
    assert!(!has_main_function_in_content("void mainHelper() {}"));
    assert!(!has_main_function_in_content("void runMain() {}"));
}

#[test]
fn test_has_main_function_rejects_single_line_comment() {
    assert!(!has_main_function_in_content("// void main() {}"));
    assert!(!has_main_function_in_content("  // void main() {}"));
    assert!(!has_main_function_in_content("/// void main() {}"));
}

#[test]
fn test_has_main_function_realistic_file() {
    let content = r#"
import 'package:flutter/material.dart';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: Scaffold(body: Text('Hello')),
    );
  }
}
"#;
    assert!(has_main_function_in_content(content));
}

#[test]
fn test_has_main_function_no_main() {
    let content = r#"
import 'package:flutter/material.dart';

class MyWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container();
  }
}
"#;
    assert!(!has_main_function_in_content(content));
}

#[test]
fn test_has_main_function_file_not_found() {
    let path = Path::new("/nonexistent/file.dart");
    assert!(!has_main_function(path));
}

#[test]
fn test_has_main_function_reads_file() {
    let temp = TempDir::new().unwrap();
    let dart_file = temp.path().join("main.dart");

    fs::write(&dart_file, "void main() {}").unwrap();
    assert!(has_main_function(&dart_file));

    fs::write(&dart_file, "void helper() {}").unwrap();
    assert!(!has_main_function(&dart_file));
}
```

### Notes

- Uses `std::sync::LazyLock` for lazy regex compilation (Rust 1.80+)
- The regex is intentionally permissive to catch valid entry points
- False positives (main in comments/strings) are acceptable - users can type custom paths
- Single-line comment detection uses `^[^/\n]*` to reject lines starting with `//`
- Multi-line comment detection is not implemented (acceptable edge case)

### Edge Cases

| Input | Expected | Notes |
|-------|----------|-------|
| `void main() {}` | true | Standard |
| `main() {}` | true | Implicit return |
| `Future<void> main() async {}` | true | Async main |
| `// void main() {}` | false | Commented out |
| `/* void main() {} */` | true* | Multi-line comment (false positive, acceptable) |
| `String s = "void main()";` | true* | String literal (false positive, acceptable) |
| `void _main() {}` | false | Private function |
| `void notmain() {}` | false | Different name |

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/core/discovery.rs` | Added `has_main_function_in_content()` and `has_main_function()` with LazyLock regex, plus 10 unit tests |

### Notable Decisions/Tradeoffs

1. **Regex Pattern**: Used `^[^/\n]*\b(?:void|Future<void>|FutureOr<void>)?\s*main\s*\(` to detect main() functions while filtering out single-line comments. The pattern intentionally allows false positives for multi-line comments and strings, as specified in the task requirements.

2. **LazyLock Usage**: Used `std::sync::LazyLock` for compile-once regex initialization, which is more efficient than compiling on every call and follows Rust best practices for static regex patterns.

3. **Error Handling**: `has_main_function()` returns `false` for file read errors, making it safe to use on potentially non-existent paths without additional error handling.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib discovery::tests::test_has_main` - Passed (10 tests)
- `cargo clippy -- -D warnings` - Passed

All 10 specified unit tests were added and pass:
- `test_has_main_function_void_main` - Tests standard `void main()` patterns
- `test_has_main_function_implicit_return` - Tests `main()` without return type
- `test_has_main_function_async` - Tests async `Future<void> main()` patterns
- `test_has_main_function_with_whitespace` - Tests various whitespace variations
- `test_has_main_function_rejects_non_main` - Tests rejection of non-main functions
- `test_has_main_function_rejects_single_line_comment` - Tests comment filtering
- `test_has_main_function_realistic_file` - Tests realistic Flutter app content
- `test_has_main_function_no_main` - Tests content without main function
- `test_has_main_function_file_not_found` - Tests missing file handling
- `test_has_main_function_reads_file` - Tests file reading and content checking

### Risks/Limitations

1. **False Positives**: The regex may match `main()` functions inside multi-line comments or string literals. This is an acceptable tradeoff as documented in the task, since users can always manually type a custom path in the UI.

2. **Regex Complexity**: The pattern relies on `^[^/\n]*` to filter single-line comments, which assumes `/` only appears in comments at the start of lines. This works for standard Dart code but could theoretically have edge cases with unusual formatting.
