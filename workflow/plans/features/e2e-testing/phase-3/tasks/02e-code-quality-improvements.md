## Task: Code Quality Improvements for pty_utils

**Objective**: Address minor code quality issues identified in the review: documentation, magic numbers, trait derives, and binary path resolution.

**Depends on**: 02-pty-test-utilities

### Scope

- `tests/e2e/pty_utils.rs`: Various improvements

### Details

This task bundles several minor improvements for efficiency:

#### 1. Add Method Documentation

Add `///` doc comments to all public methods, especially:
- `spawn()` / `spawn_with_args()`
- `expect_header()`, `expect_running()`, `expect_reloading()`
- `send_key()`, `send_special()`
- `capture_screen()`
- `quit()`, `kill()`

**Example:**
```rust
/// Spawn fdemon in a PTY for the given Flutter project.
///
/// # Arguments
/// * `project_path` - Path to a Flutter project directory
///
/// # Returns
/// A new `FdemonSession` ready for interaction.
///
/// # Errors
/// Returns error if fdemon binary not found or spawn fails.
pub fn spawn(project_path: &Path) -> PtyResult<Self> {
```

#### 2. Extract Magic Numbers to Constants

Replace hardcoded sleep durations with named constants:

```rust
/// Default timeout for expect operations
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Time to wait for graceful quit before force-killing
const QUIT_GRACE_PERIOD_MS: u64 = 500;

/// Time to wait between kill attempts
const KILL_RETRY_DELAY_MS: u64 = 100;

/// Short delay for screen capture
const CAPTURE_DELAY_MS: u64 = 100;
```

#### 3. Add PartialEq and Eq to SpecialKey

Improve testability by deriving comparison traits:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    // ...
}
```

This enables:
```rust
assert_eq!(key, SpecialKey::Enter);
```

#### 4. Improve Binary Path Resolution

Make binary discovery more robust with helpful error messages:

```rust
fn find_fdemon_binary() -> PtyResult<String> {
    // 1. Check CARGO_BIN_EXE_fdemon (set by cargo test)
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_fdemon") {
        if Path::new(&path).exists() {
            return Ok(path);
        }
    }

    // 2. Check release build
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let release = format!("{}/target/release/fdemon", manifest_dir);
    if Path::new(&release).exists() {
        return Ok(release);
    }

    // 3. Check debug build
    let debug = format!("{}/target/debug/fdemon", manifest_dir);
    if Path::new(&debug).exists() {
        return Ok(debug);
    }

    Err("fdemon binary not found. Run `cargo build` first.".into())
}
```

### Acceptance Criteria

1. All public methods have `///` doc comments
2. No magic numbers in `sleep()` calls - all use named constants
3. `SpecialKey` derives `PartialEq, Eq`
4. Binary path resolution checks existence and provides helpful error
5. `cargo doc --test` generates documentation without warnings

### Testing

```bash
# Verify documentation compiles
cargo doc --document-private-items --no-deps

# Verify SpecialKey comparison works
cargo test --test e2e test_special_key
```

```rust
#[test]
fn test_special_key_equality() {
    assert_eq!(SpecialKey::Enter, SpecialKey::Enter);
    assert_ne!(SpecialKey::Enter, SpecialKey::Escape);
}
```

### Notes

- These are low-effort, high-value improvements
- Documentation helps future contributors understand the API
- Constants make timeouts easier to tune for different environments

### Review Source

- Code Quality Inspector: "Missing doc comments", "Magic numbers", "SpecialKey derives"
- ACTION_ITEMS.md Issues #5, #6, #7, #8

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/e2e/pty_utils.rs` | Added comprehensive doc comments to all public methods and types; extracted magic numbers to named constants; added `PartialEq` and `Eq` derives to `SpecialKey`; improved binary path resolution with existence checks and helpful error messages |

### Notable Decisions/Tradeoffs

1. **Test-specific constants**: Added `TEST_STARTUP_DELAY_MS` and `TEST_KEY_PROCESSING_DELAY_MS` for test code to ensure no magic numbers anywhere in the file, even in test functions.
2. **Documentation style**: Used standard Rust doc comment conventions with `# Arguments`, `# Returns`, `# Errors`, and `# Example` sections for clarity.
3. **Binary resolution function**: Extracted `find_fdemon_binary()` as a separate function for better separation of concerns and testability. Checks in order: `CARGO_BIN_EXE_fdemon` env var, release build, then debug build.
4. **SpecialKey documentation**: Added detailed doc comments for the enum and each variant to explain their purpose and ANSI escape sequences.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (0.21s)
- `cargo doc --document-private-items --no-deps` - Passed (generated documentation successfully, 12 pre-existing warnings unrelated to this task)
- `cargo clippy -- -D warnings` - Passed (0.14s)
- `cargo test --test e2e test_special_key` - Passed (3 tests: test_special_key_bytes, test_special_key_function_keys, test_special_key_equality)

### Risks/Limitations

1. **None identified**: All changes are non-breaking improvements to code quality and maintainability. The existing API surface remains unchanged.
