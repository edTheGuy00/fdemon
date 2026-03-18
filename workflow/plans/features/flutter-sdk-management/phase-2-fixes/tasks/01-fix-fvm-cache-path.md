## Task: Fix FVM_CACHE_PATH Mismatch in Removal Safety Check

**Objective**: Unify the FVM cache path resolution so the removal safety check uses the same logic as the cache scanner, and fix the `unwrap_or_default()` hazard on `dirs::home_dir()`.

**Depends on**: None

**Severity**: CRITICAL — version removal is broken for all users with `FVM_CACHE_PATH` set

### Scope

- `crates/fdemon-daemon/src/flutter_sdk/cache_scanner.rs`: Make `resolve_fvm_cache_path()` public
- `crates/fdemon-daemon/src/flutter_sdk/mod.rs`: Re-export `resolve_fvm_cache_path`
- `crates/fdemon-app/src/actions/mod.rs`: Replace hardcoded safety check with call to `resolve_fvm_cache_path()`

### Details

#### The Bug

**File:** `crates/fdemon-app/src/actions/mod.rs`, lines 776-793

The cache scanner in `cache_scanner.rs` checks `FVM_CACHE_PATH` env var first, then falls back to `~/fvm/versions/`. The removal safety check in `actions/mod.rs` hardcodes only `~/fvm/versions/`:

```rust
// Current broken code in actions/mod.rs:778-786
let fvm_cache = dirs::home_dir()
    .unwrap_or_default()       // BUG 1: empty PathBuf when HOME unset
    .join("fvm")
    .join("versions");
if !path.starts_with(&fvm_cache) {  // BUG 2: ignores FVM_CACHE_PATH
    return Err(fdemon_core::Error::config(format!(
        "Refusing to remove path outside FVM cache: {}",
        path.display()
    )));
}
```

When a user has `FVM_CACHE_PATH=/custom/path`, the scanner returns `InstalledSdk` entries rooted under `/custom/path`, but the safety check rejects them as "outside FVM cache" because it only knows about `~/fvm/versions/`.

Additionally, `dirs::home_dir().unwrap_or_default()` produces an empty `PathBuf` when `HOME` is unset, which makes `fvm_cache` a relative path `fvm/versions` — the `starts_with` check then fails for any absolute SDK path.

#### Root Cause: Three Independent Implementations

There are currently three copies of FVM cache resolution:

1. `cache_scanner::resolve_fvm_cache_path()` — private, checks env var + dir existence (**best version**)
2. `version_managers::resolve_fvm_cache()` — private, checks env var, no existence check
3. `actions/mod.rs` inline — checks neither env var nor home_dir failure (**broken**)

#### The Fix

**Step 1: Make `resolve_fvm_cache_path` public in `cache_scanner.rs`**

Change line 46:
```rust
// Before
fn resolve_fvm_cache_path() -> Option<PathBuf> {

// After
pub fn resolve_fvm_cache_path() -> Option<PathBuf> {
```

**Step 2: Add to re-export list in `flutter_sdk/mod.rs`**

Add `resolve_fvm_cache_path` to the existing `pub use cache_scanner::{...}` line.

**Step 3: Replace the inline safety check in `actions/mod.rs`**

```rust
// Before (lines 778-786)
let fvm_cache = dirs::home_dir()
    .unwrap_or_default()
    .join("fvm")
    .join("versions");
if !path.starts_with(&fvm_cache) { ... }

// After
let fvm_cache = fdemon_daemon::flutter_sdk::resolve_fvm_cache_path()
    .ok_or_else(|| fdemon_core::Error::config(
        "FVM cache directory not found; cannot safely remove version".to_string()
    ))?;
if !path.starts_with(&fvm_cache) {
    return Err(fdemon_core::Error::config(format!(
        "Refusing to remove path outside FVM cache: {}",
        path.display()
    )));
}
```

This eliminates the `unwrap_or_default()` hazard and ensures the removal guard uses the exact same path the scanner used to discover the version.

**Step 4 (optional): Consider unifying `version_managers::resolve_fvm_cache` too**

`version_managers.rs:49-57` has its own private copy. If practical, have it call the now-public `resolve_fvm_cache_path()` instead. This reduces from 3 copies to 1 canonical source. However, `version_managers` does NOT check `.is_dir()` on the env var path (it trusts the value), so verify this difference is acceptable or adjust.

### Acceptance Criteria

1. `resolve_fvm_cache_path()` is `pub` in `cache_scanner.rs` and re-exported from `fdemon_daemon::flutter_sdk`
2. The removal safety check in `actions/mod.rs` calls `resolve_fvm_cache_path()` instead of hardcoding `~/fvm/versions/`
3. When `HOME` is unset and `FVM_CACHE_PATH` is unset, removal returns a config error (not a silent empty-path check)
4. When `FVM_CACHE_PATH=/custom/path` is set, removal of versions under that path succeeds
5. `cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

### Testing

Add a test in `actions/mod.rs` (or the existing test module for `remove_flutter_version`):

```rust
#[test]
fn test_remove_rejects_path_outside_fvm_cache() {
    // Test that paths outside the FVM cache are rejected
    let result = remove_flutter_version(PathBuf::from("/tmp/not-fvm/some-sdk"));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("outside FVM cache")
        || err.to_string().contains("not found"));
}
```

The FVM_CACHE_PATH env var test is best done as a manual test or integration test since it requires env var manipulation in a way that's safe for parallel test execution (use `temp_env` or `serial_test` if available).

### Notes

- `resolve_fvm_cache_path` in `cache_scanner.rs` is the best canonical version because it checks both the env var AND verifies the path is a directory (`.is_dir()`).
- `fdemon-app` already depends on `fdemon-daemon`, so the cross-crate call introduces no new dependency.
- The `dirs` import in `actions/mod.rs` may become unused after this fix — remove it if so.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/flutter_sdk/cache_scanner.rs` | Changed `fn resolve_fvm_cache_path` to `pub fn resolve_fvm_cache_path` |
| `crates/fdemon-daemon/src/flutter_sdk/mod.rs` | Added `resolve_fvm_cache_path` to `pub use cache_scanner::{...}` re-export list |
| `crates/fdemon-app/src/actions/mod.rs` | Extracted `remove_flutter_version_path()` helper that calls `resolve_fvm_cache_path()`; replaced inline `dirs::home_dir().unwrap_or_default()` logic with call to the new helper; added `test_remove_rejects_path_outside_fvm_cache` test |

### Notable Decisions/Tradeoffs

1. **Extracted helper function**: Rather than only patching the inline closure, extracted a `remove_flutter_version_path()` free function. This made the closure one line, made the safety logic unit-testable, and keeps the async dispatch code clean.

2. **`dirs` dependency retained in Cargo.toml**: The `dirs` crate remains in `fdemon-app/Cargo.toml` because it may be used elsewhere in the crate at compile time. Clippy confirmed zero warnings after the inline usage was removed — no unused-extern-crate lint was triggered, so no Cargo.toml change is needed.

3. **`ok_or_else` error for missing FVM cache**: When neither `FVM_CACHE_PATH` nor `~/fvm/versions/` exists, the function returns a config error rather than silently passing an empty-path check, satisfying acceptance criterion 3.

### Testing Performed

- `cargo check -p fdemon-daemon` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1772 tests, 0 failed)
- `cargo clippy -p fdemon-app -p fdemon-daemon -- -D warnings` - Passed (0 warnings)
- `cargo fmt --all && cargo check --workspace` - Passed

### Risks/Limitations

1. **`FVM_CACHE_PATH` env var test**: The task notes that testing the env-var path requires `serial_test` or `temp_env` to avoid parallel test interference. This was intentionally left as a manual/integration test per the task's guidance.
