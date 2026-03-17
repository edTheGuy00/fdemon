## Task: Minor Code Quality Fixes

**Objective**: Address the 5 minor issues from the Phase 1 review: fully-qualified Result types, missing handler tests, magic number constant, PATH test restoration, and duplicate info log.

**Depends on**: 02-refactor-locator (shares `locator.rs` test file)

**Addresses**: Review issues #5, #6, #7, #8, #9

### Scope

- `crates/fdemon-daemon/src/flutter_sdk/version_managers.rs`: Fix 7 function signatures
- `crates/fdemon-daemon/src/flutter_sdk/channel.rs`: Add named constant for git short hash length
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs`: Fix PATH restoration in test
- `crates/fdemon-app/src/engine.rs`: Remove duplicate info! log
- `crates/fdemon-app/src/handler/tests.rs`: Add tests for SdkResolved/SdkResolutionFailed handlers

### Details

#### Fix 1: Fully-Qualified Result in version_managers.rs (Issue #5)

**File:** `crates/fdemon-daemon/src/flutter_sdk/version_managers.rs`

All 7 public detection functions use `fdemon_core::error::Result<Option<PathBuf>>` despite `use fdemon_core::prelude::*;` at line 21 which brings `Result` into scope.

**Change:** Replace all 7 return types with bare `Result<Option<PathBuf>>`.

| Line | Function | Before | After |
|------|----------|--------|-------|
| 67 | `detect_fvm_modern` | `fdemon_core::error::Result<Option<PathBuf>>` | `Result<Option<PathBuf>>` |
| 130 | `detect_fvm_legacy` | `fdemon_core::error::Result<Option<PathBuf>>` | `Result<Option<PathBuf>>` |
| 212 | `detect_puro` | `fdemon_core::error::Result<Option<PathBuf>>` | `Result<Option<PathBuf>>` |
| 278 | `detect_asdf` | `fdemon_core::error::Result<Option<PathBuf>>` | `Result<Option<PathBuf>>` |
| 359 | `detect_mise` | `fdemon_core::error::Result<Option<PathBuf>>` | `Result<Option<PathBuf>>` |
| 442 | `detect_proto` | `fdemon_core::error::Result<Option<PathBuf>>` | `Result<Option<PathBuf>>` |
| 523 | `detect_flutter_wrapper` | `fdemon_core::error::Result<Option<PathBuf>>` | `Result<Option<PathBuf>>` |

#### Fix 2: Missing Tests for SdkResolved/SdkResolutionFailed (Issue #6)

**File:** `crates/fdemon-app/src/handler/tests.rs`

The `Message::SdkResolved` and `Message::SdkResolutionFailed` handlers at `update.rs` lines 2433-2447 have no test coverage. These are state transition handlers that must be tested per project standards.

```rust
#[test]
fn test_sdk_resolved_updates_state() {
    let mut state = AppState::new();
    assert!(state.resolved_sdk.is_none());
    assert!(!state.tool_availability.flutter_sdk);

    let sdk = fdemon_daemon::test_utils::fake_flutter_sdk();
    let result = update(&mut state, Message::SdkResolved { sdk });

    assert!(state.resolved_sdk.is_some());
    assert!(state.tool_availability.flutter_sdk);
    assert!(state.tool_availability.flutter_sdk_source.is_some());
    assert!(result.action.is_none());
}

#[test]
fn test_sdk_resolution_failed_clears_state() {
    let mut state = AppState::new();
    // Pre-populate SDK
    state.resolved_sdk = Some(fdemon_daemon::test_utils::fake_flutter_sdk());
    state.tool_availability.flutter_sdk = true;
    state.tool_availability.flutter_sdk_source = Some("system PATH".to_string());

    let result = update(
        &mut state,
        Message::SdkResolutionFailed {
            reason: "No SDK found".to_string(),
        },
    );

    assert!(state.resolved_sdk.is_none());
    assert!(!state.tool_availability.flutter_sdk);
    assert!(state.tool_availability.flutter_sdk_source.is_none());
    assert!(result.action.is_none());
}
```

#### Fix 3: Magic Number 7 in channel.rs (Issue #7)

**File:** `crates/fdemon-daemon/src/flutter_sdk/channel.rs`, line 64

```rust
// Before:
let short = &content[..content.len().min(7)];

// After:
/// Standard git short hash length for display.
const SHORT_HASH_LEN: usize = 7;

let short = &content[..content.len().min(SHORT_HASH_LEN)];
```

Place the constant near the top of the function or at module level (near the `detect_channel` function since it's the only consumer).

#### Fix 4: PATH Test Restoration in locator.rs (Issue #8)

**File:** `crates/fdemon-daemon/src/flutter_sdk/locator.rs`

The `test_all_strategies_fail_returns_flutter_not_found` test (lines 632-641) sets `PATH` to a temp dir but then removes it entirely instead of restoring the original value.

**Note:** After Task 02 (refactor-locator), line numbers will have changed. Find the test by name.

```rust
// Before:
std::env::set_var("PATH", tmp.path());
std::env::remove_var("FLUTTER_ROOT");
let result = find_flutter_sdk(tmp.path(), None);
std::env::remove_var("PATH");

// After:
let original_path = std::env::var_os("PATH");
std::env::set_var("PATH", tmp.path());
std::env::remove_var("FLUTTER_ROOT");
let result = find_flutter_sdk(tmp.path(), None);
// Restore PATH to its original value
match original_path {
    Some(v) => std::env::set_var("PATH", v),
    None => std::env::remove_var("PATH"),
}
```

#### Fix 5: Duplicate info! Log (Issue #9)

**File:** `crates/fdemon-app/src/engine.rs`, lines 206-211

SDK resolution is logged both in `find_flutter_sdk` (locator.rs — structured log with `source`, `version`, `path` fields) and in `Engine::new()` (format string with the same data). Remove the `Engine::new()` duplicate.

```rust
// Remove this block from Engine::new():
info!(
    "Flutter SDK resolved via {}: {} at {}",
    sdk.source,
    sdk.version,
    sdk.root.display()
);

// Keep only the structured log inside find_flutter_sdk / try_resolve_sdk (locator.rs):
info!(
    source = %sdk.source,
    version = %sdk.version,
    path = %sdk.root.display(),
    "Flutter SDK resolved"
);
```

The `Engine::new()` `Ok(sdk)` arm should retain the `Some(sdk)` assignment but drop the duplicate `info!` call. The `Err(e)` arm's `warn!` is NOT a duplicate — keep it.

### Acceptance Criteria

1. All 7 version_managers.rs functions use bare `Result<Option<PathBuf>>` in their signatures
2. Tests exist for `Message::SdkResolved` and `Message::SdkResolutionFailed` verifying state transitions
3. `SHORT_HASH_LEN` constant replaces magic number `7` in channel.rs
4. `test_all_strategies_fail_returns_flutter_not_found` saves and restores original PATH
5. Only one `info!` log line appears for SDK resolution on startup (the structured log in locator.rs)
6. `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

### Testing

- The Result type changes are signature-only — all existing tests pass without modification.
- The two new handler tests verify state transitions (see Fix 2 above).
- The SHORT_HASH_LEN change is covered by existing `test_detect_channel_detached_head`.
- The PATH fix is verified by the existing test continuing to pass.
- The duplicate log removal is verified by manual inspection or by checking startup log output.

### Notes

- This task depends on Task 02 because both touch `locator.rs` (Task 02 refactors the main function, this task fixes a test). Run after Task 02 to avoid merge conflicts.
- The version_managers.rs signature changes are mechanical and risk-free.
- The `engine.rs` log removal must be done carefully — only remove the `info!` inside the `Ok(sdk)` arm, not the `warn!` inside the `Err(e)` arm.

---

## Completion Summary

**Status:** Not Started
