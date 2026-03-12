## Task: Move `parse_min_level` to `fdemon-core`

**Objective**: Relocate the `parse_min_level` function from `fdemon-daemon` to `fdemon-core` where it belongs alongside `LogLevel`.

**Depends on**: None

**Review Issue**: #6 (MINOR)

### Scope

- `crates/fdemon-core/src/types.rs`: Add `LogLevel::from_level_str()` method
- `crates/fdemon-daemon/src/native_logs/mod.rs`: Remove `parse_min_level`, re-export or update call sites
- `crates/fdemon-daemon/src/native_logs/ios.rs`: Update 2 call sites
- `crates/fdemon-daemon/src/native_logs/macos.rs`: Update call site (if task 01 is done first)
- `crates/fdemon-app/src/handler/update.rs`: Update call site (~line 1941)
- `crates/fdemon-core/src/lib.rs`: Export the new method if needed

### Details

`parse_min_level` (daemon/native_logs/mod.rs:145-153) converts a `&str` to `Option<LogLevel>`. It operates entirely on core types — its only import is `LogLevel`. The problematic call site is in `fdemon-app/handler/update.rs:1941` where the app layer reaches into `fdemon_daemon::native_logs::parse_min_level` for a pure utility function. This violates the layer boundary (app should not depend on daemon for type conversions).

**Current function:**
```rust
pub fn parse_min_level(level: &str) -> Option<LogLevel> {
    match level.to_lowercase().as_str() {
        "verbose" | "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warning" => Some(LogLevel::Warning),
        "error" => Some(LogLevel::Error),
        _ => None,
    }
}
```

**Target: `impl LogLevel` in `fdemon-core/src/types.rs`:**
```rust
impl LogLevel {
    /// Parses a level string (case-insensitive) into a `LogLevel`.
    ///
    /// Accepts: "verbose", "debug", "info", "warning", "error".
    /// Returns `None` for unrecognised strings.
    pub fn from_level_str(s: &str) -> Option<LogLevel> {
        match s.to_lowercase().as_str() {
            "verbose" | "debug" => Some(LogLevel::Debug),
            "info" => Some(LogLevel::Info),
            "warning" => Some(LogLevel::Warning),
            "error" => Some(LogLevel::Error),
            _ => None,
        }
    }
}
```

**Update call sites:**

- `fdemon-daemon/src/native_logs/ios.rs:162,280`: `super::parse_min_level(...)` → `LogLevel::from_level_str(...)`
- `fdemon-daemon/src/native_logs/mod.rs`: Remove `parse_min_level` (or keep as deprecated re-export)
- `fdemon-app/src/handler/update.rs:1941`: `fdemon_daemon::native_logs::parse_min_level(...)` → `LogLevel::from_level_str(...)`

### Acceptance Criteria

1. `parse_min_level` logic lives in `fdemon-core` as `LogLevel::from_level_str()`
2. No app-layer code calls into `fdemon_daemon` for this conversion
3. All daemon-internal call sites updated
4. Existing tests pass; unit tests for `from_level_str` exist in `fdemon-core`
5. `cargo check -p fdemon-core` passes independently

### Testing

Move or duplicate existing tests for `parse_min_level` to `fdemon-core`:

```rust
#[test]
fn test_from_level_str() {
    assert_eq!(LogLevel::from_level_str("debug"), Some(LogLevel::Debug));
    assert_eq!(LogLevel::from_level_str("verbose"), Some(LogLevel::Debug));
    assert_eq!(LogLevel::from_level_str("INFO"), Some(LogLevel::Info));
    assert_eq!(LogLevel::from_level_str("Warning"), Some(LogLevel::Warning));
    assert_eq!(LogLevel::from_level_str("ERROR"), Some(LogLevel::Error));
    assert_eq!(LogLevel::from_level_str("unknown"), None);
    assert_eq!(LogLevel::from_level_str(""), None);
}
```

### Notes

- Consider implementing `FromStr` for `LogLevel` instead, but `from_level_str` is simpler since the accepted strings don't match Rust's standard `FromStr` convention (e.g., "verbose" maps to `Debug`)
- If this task runs before task 01, the macOS fix can use `LogLevel::from_level_str()` directly
