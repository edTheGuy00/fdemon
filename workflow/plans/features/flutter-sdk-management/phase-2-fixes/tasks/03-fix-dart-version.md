## Task: Fix dart_version Not Refreshed After Version Switch

**Objective**: Update `handle_switch_completed` to refresh `sdk_info.dart_version` after a version switch so the SDK info pane shows the correct Dart version for the newly active SDK.

**Depends on**: None

**Severity**: CRITICAL — stale data displayed after every version switch

### Scope

- `crates/fdemon-app/src/flutter_version/state.rs`: Change `read_dart_version` visibility to `pub(crate)`
- `crates/fdemon-app/src/handler/flutter_version/actions.rs`: Add dart_version update in `handle_switch_completed`

### Details

#### The Bug

**File:** `crates/fdemon-app/src/handler/flutter_version/actions.rs`, lines 86-95

```rust
pub fn handle_switch_completed(state: &mut AppState, version: String) -> UpdateResult {
    state.flutter_version_state.status_message = Some(format!("Switched to {version}"));

    // Refresh the SDK info pane with the new resolved SDK
    state.flutter_version_state.sdk_info.resolved_sdk = state.resolved_sdk.clone();

    // Re-scan to update is_active markers
    let active_sdk_root = state.resolved_sdk.as_ref().map(|sdk| sdk.root.clone());
    UpdateResult::action(UpdateAction::ScanInstalledSdks { active_sdk_root })
}
```

Line 90 updates `sdk_info.resolved_sdk` to the new SDK, but `sdk_info.dart_version` is never touched. After switching, the panel displays the Dart version from the SDK that was active when the panel was first opened.

#### `SdkInfoState` Definition

**File:** `crates/fdemon-app/src/flutter_version/state.rs`, lines 72-78

```rust
pub struct SdkInfoState {
    pub resolved_sdk: Option<FlutterSdk>,
    pub dart_version: Option<String>,
}
```

Both fields are `pub`, so they can be written from the handler.

#### `read_dart_version` — Private, Needs Visibility Bump

**File:** `crates/fdemon-app/src/flutter_version/state.rs`, lines 58-68

```rust
fn read_dart_version(sdk_root: &Path) -> Option<String> {
    let version_path = sdk_root
        .join("bin")
        .join("cache")
        .join("dart-sdk")
        .join("version");
    std::fs::read_to_string(&version_path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
```

This is a bare `fn` (private). The module structure is:
- `flutter_version/mod.rs` has `mod state; pub use state::*;`
- `pub use state::*` only re-exports `pub` items, so `read_dart_version` is invisible outside `state.rs`

#### The Fix

**Step 1: Bump visibility in `state.rs`**

```rust
// Before
fn read_dart_version(sdk_root: &Path) -> Option<String> {

// After
pub(crate) fn read_dart_version(sdk_root: &Path) -> Option<String> {
```

**Step 2: Add dart_version refresh in `handle_switch_completed`**

```rust
pub fn handle_switch_completed(state: &mut AppState, version: String) -> UpdateResult {
    state.flutter_version_state.status_message = Some(format!("Switched to {version}"));

    // Refresh the SDK info pane with the new resolved SDK
    state.flutter_version_state.sdk_info.resolved_sdk = state.resolved_sdk.clone();

    // Refresh dart_version from the new SDK's dart-sdk/version file
    state.flutter_version_state.sdk_info.dart_version = state
        .resolved_sdk
        .as_ref()
        .and_then(|sdk| crate::flutter_version::read_dart_version(&sdk.root));

    // Re-scan to update is_active markers
    let active_sdk_root = state.resolved_sdk.as_ref().map(|sdk| sdk.root.clone());
    UpdateResult::action(UpdateAction::ScanInstalledSdks { active_sdk_root })
}
```

The import path `crate::flutter_version::read_dart_version` works because `flutter_version/mod.rs` has `pub use state::*` and the function will now be `pub(crate)`.

#### Why Synchronous Read Is Acceptable

The file (`<sdk>/bin/cache/dart-sdk/version`) is under 100 bytes. `FlutterVersionState::new()` already reads it synchronously on panel open — the existing code has an explicit comment documenting this as acceptable. Adding the same call in `handle_switch_completed` is consistent with the established pattern.

### Acceptance Criteria

1. `read_dart_version` is `pub(crate)` in `state.rs`
2. `handle_switch_completed` updates `sdk_info.dart_version` using `read_dart_version` on the new SDK root
3. After switching versions, the SDK info pane shows the correct Dart version
4. When `resolved_sdk` is `None` (shouldn't happen in practice, but for safety), `dart_version` is set to `None`
5. `cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

### Testing

Add/update a test in the existing `actions.rs` test module:

```rust
#[test]
fn test_switch_completed_updates_dart_version() {
    let mut state = make_test_state();
    // Set up a resolved SDK with a known root
    let sdk = fake_flutter_sdk();
    state.resolved_sdk = Some(sdk.clone());
    state.show_flutter_version();

    // Simulate a dart_version from the original SDK
    state.flutter_version_state.sdk_info.dart_version = Some("3.3.0".to_string());

    // After switch, dart_version should be refreshed (will be None in test
    // since the fake SDK path doesn't have a real dart-sdk/version file)
    let result = handle_switch_completed(&mut state, "3.22.0".to_string());

    // The dart_version field was updated (even if to None in test environment)
    // The key assertion is that the code path runs without error
    assert!(result.action.is_some()); // ScanInstalledSdks returned
    // resolved_sdk was copied to sdk_info
    assert!(state.flutter_version_state.sdk_info.resolved_sdk.is_some());
}
```

For a more precise test, create a temp directory with a `bin/cache/dart-sdk/version` file containing "3.4.0\n", set it as the SDK root, and verify `dart_version` reads correctly:

```rust
#[test]
fn test_switch_completed_reads_new_dart_version() {
    let dir = tempfile::tempdir().unwrap();
    let dart_version_dir = dir.path().join("bin/cache/dart-sdk");
    std::fs::create_dir_all(&dart_version_dir).unwrap();
    std::fs::write(dart_version_dir.join("version"), "3.4.0\n").unwrap();

    let mut state = make_test_state();
    let mut sdk = fake_flutter_sdk();
    sdk.root = dir.path().to_path_buf();
    state.resolved_sdk = Some(sdk);
    state.show_flutter_version();
    state.flutter_version_state.sdk_info.dart_version = Some("3.3.0".to_string());

    handle_switch_completed(&mut state, "3.22.0".to_string());

    assert_eq!(
        state.flutter_version_state.sdk_info.dart_version.as_deref(),
        Some("3.4.0")
    );
}
```

### Notes

- This is a 2-line fix (1 visibility change + 1 new line in handler). Minimal blast radius.
- The `pub(crate)` visibility is sufficient — `read_dart_version` does not need to be visible outside `fdemon-app`.
