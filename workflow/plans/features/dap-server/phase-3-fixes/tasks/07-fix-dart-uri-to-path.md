## Task: Fix `dart_uri_to_path` for Windows Paths

**Objective**: Fix the `dart_uri_to_path` function which uses a fragile `strip_prefix("file://")` that produces incorrect paths on Windows (`file:///C:/path` → `/C:/path` instead of `C:/path`). Either use proper URL parsing or add explicit platform documentation and tests.

**Depends on**: None

**Estimated Time**: 1–2 hours

**Severity**: MAJOR — Windows users will get broken source paths in DAP stack traces.

### Scope

- `crates/fdemon-dap/src/adapter/stack.rs`: Fix `dart_uri_to_path` (line 323) and add tests

### Details

#### Current Implementation

```rust
// stack.rs:323-333
pub fn dart_uri_to_path(uri: &str) -> Option<String> {
    if let Some(path) = uri.strip_prefix("file://") {
        Some(path.to_string())
    } else if uri.starts_with("dart:") || uri.starts_with("package:") {
        None
    } else {
        None
    }
}
```

#### Problem

`file://` URIs for absolute paths have three slashes: `file:///path`. After stripping `"file://"` (two slashes):
- Unix: `file:///home/user/app.dart` → `/home/user/app.dart` — **accidentally correct** (the third `/` becomes the path root)
- Windows: `file:///C:/Users/app.dart` → `/C:/Users/app.dart` — **wrong** (leading `/` before drive letter)

#### Fix Options

**Option A (recommended): Use `url::Url::parse().to_file_path()`**

```rust
pub fn dart_uri_to_path(uri: &str) -> Option<String> {
    if uri.starts_with("file://") {
        url::Url::parse(uri)
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .map(|p| p.to_string_lossy().into_owned())
    } else if uri.starts_with("dart:") || uri.starts_with("package:") {
        None
    } else {
        None
    }
}
```

Check if the `url` crate is already a dependency. If not, it's a lightweight, well-maintained crate that handles all edge cases (UNC paths, percent-encoding, etc.).

**Option B: Manual fix with platform-aware stripping**

```rust
pub fn dart_uri_to_path(uri: &str) -> Option<String> {
    if let Some(path) = uri.strip_prefix("file:///") {
        // Strip three slashes for absolute paths
        if cfg!(windows) && path.chars().nth(1) == Some(':') {
            // Windows drive letter: file:///C:/path → C:/path
            Some(path.to_string())
        } else {
            // Unix: file:///home/path → /home/path
            Some(format!("/{}", path))
        }
    } else {
        // dart:, package:, or unknown schemes
        None
    }
}
```

Option A is preferred — it handles edge cases (percent-encoded paths, UNC paths) that manual parsing would miss.

#### Check `url` crate availability

```bash
grep -r "^url" crates/fdemon-dap/Cargo.toml
```

If not present, add `url = "2"` to `[dependencies]`.

### Acceptance Criteria

1. `dart_uri_to_path("file:///home/user/app.dart")` → `Some("/home/user/app.dart")` (Unix)
2. `dart_uri_to_path("file:///C:/Users/app.dart")` → `Some("C:\\Users\\app.dart")` or `Some("C:/Users/app.dart")` (Windows)
3. `dart_uri_to_path("file:///tmp/app.dart")` → `Some("/tmp/app.dart")`
4. `dart_uri_to_path("dart:core/list.dart")` → `None`
5. `dart_uri_to_path("package:my_app/main.dart")` → `None`
6. Percent-encoded paths decoded correctly: `dart_uri_to_path("file:///home/my%20app/main.dart")` → `Some("/home/my app/main.dart")`
7. All existing tests pass
8. `cargo clippy --workspace` passes

### Testing

Add comprehensive test cases:

```rust
#[test]
fn test_dart_uri_to_path_unix_absolute() {
    assert_eq!(
        dart_uri_to_path("file:///home/user/app/lib/main.dart"),
        Some("/home/user/app/lib/main.dart".to_string())
    );
}

#[test]
fn test_dart_uri_to_path_windows_drive_letter() {
    // This test verifies the fix — previously returned "/C:/Users/..."
    let result = dart_uri_to_path("file:///C:/Users/app/lib/main.dart");
    assert!(result.is_some());
    let path = result.unwrap();
    assert!(!path.starts_with("/C:"), "Should not have leading / before drive letter");
}

#[test]
fn test_dart_uri_to_path_percent_encoded() {
    assert_eq!(
        dart_uri_to_path("file:///home/my%20project/main.dart"),
        Some("/home/my project/main.dart".to_string())
    );
}
```

### Notes

- If using Option A with the `url` crate, note that `to_file_path()` returns platform-specific `PathBuf`. On Unix it produces forward slashes; on Windows it produces backslashes. The test assertions should account for this.
- The existing test `test_dart_uri_to_path_file_uri_strips_prefix_only` asserts the current behavior and must be updated if the implementation changes.
- Flutter's Dart VM Service always uses `file:///` (three slashes) for absolute paths, so the two-slash `file://hostname/path` form is unlikely but should be handled gracefully.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` | Added `url = "2"` to `[workspace.dependencies]` |
| `crates/fdemon-dap/Cargo.toml` | Added `url.workspace = true` to `[dependencies]` |
| `crates/fdemon-dap/src/adapter/stack.rs` | Fixed `dart_uri_to_path`, updated doc comment, added 10 new tests |

### Notable Decisions/Tradeoffs

1. **Option A (url crate) chosen**: `url::Url::parse().to_file_path()` correctly handles percent-encoding, Windows drive letters, and UNC paths with no manual string manipulation.

2. **Windows drive letter behavior on Unix**: On macOS/Linux, `url::Url::parse("file:///C:/...")` succeeds (empty host, path `/C:/...`) so `to_file_path()` returns `Some("/C:/Users/...")` rather than `None`. This is an inherent platform limitation documented in the function's doc comment and the `test_dart_uri_to_path_windows_drive_letter` test. The key fix is on Windows, where the old code returned `/C:/...` (broken) and the new code correctly returns `C:\...`.

3. **`file://hostname/path` URIs return `None`**: The `url` crate correctly rejects non-empty host components in `to_file_path()`. Flutter's VM Service never emits such URIs, so this is safe.

4. **Existing test `test_dart_uri_to_path_file_uri_strips_prefix_only` unchanged**: The function's Unix behavior (`/tmp/app.dart`) is preserved; only the comment wording was updated from "stripping prefix" to "converting URI".

### Testing Performed

- `cargo test -p fdemon-dap --lib adapter::stack` — PASS (50 tests, all green)
- `cargo check --workspace` — PASS
- `cargo fmt --all` — PASS (no formatting changes)
- `cargo clippy --workspace -- -D warnings` — PASS (zero warnings)

### Risks/Limitations

1. **Windows drive letters on Unix**: If a Windows-generated `file:///C:/...` URI is sent to a Unix DAP adapter, the returned path `/C:/Users/...` won't resolve on the filesystem. This is a cross-platform limitation documented in the function and acceptable since Flutter development on Unix always produces Unix paths.

2. **Pre-existing test failures (unrelated)**: 3–5 tests in `server` and `transport` modules fail from changes made by other tasks (01, 03, 04). These are not caused by this task.
