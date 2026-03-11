## Task: Fix `derive_ios_process_name` — Always Return `"Runner"`

**Objective**: Fix iOS native log capture so logs actually appear. Currently `derive_ios_process_name` returns the last component of the bundle ID (e.g., `"flutterDeamonSample"`), but iOS Flutter apps always use `"Runner"` as the process name. The resulting `--predicate 'process == "flutterDeamonSample"'` matches nothing.

**Depends on**: None

**Review Issue:** #1 (Critical/Blocking)

### Scope

- `crates/fdemon-app/src/actions/native_logs.rs`: Fix `derive_ios_process_name` function (lines 265-277)

### Details

#### Problem

The current implementation delegates to `derive_macos_process_name`, which extracts the last dot-separated component of the bundle ID:

```rust
/// Derive the iOS process name from the app bundle identifier.
/// iOS uses the same naming convention as macOS.
fn derive_ios_process_name(app_id: &Option<String>) -> String {
    // iOS Flutter apps use the same process name derivation as macOS
    derive_macos_process_name(app_id)
}
```

`derive_macos_process_name` uses `rsplit('.').next()` to extract e.g. `"flutterDeamonSample"` from `"com.example.flutterDeamonSample"`. This is correct for macOS but wrong for iOS.

On iOS, Flutter apps are always built with the Xcode target name `"Runner"` — confirmed by `example/app2/ios/Runner.xcodeproj/project.pbxproj` which shows `PRODUCT_NAME = "$(TARGET_NAME)"` (resolves to `Runner`). The process name in `log stream` output and `idevicesyslog` is always `"Runner"`, not the bundle ID component.

Both downstream consumers are affected:
- **Simulator** (`ios.rs:254`): `--predicate 'process == "flutterDeamonSample"'` matches zero log lines
- **Physical** (`ios.rs:145`): `idevicesyslog -p flutterDeamonSample` also produces no output

#### Fix

Replace the body of `derive_ios_process_name` to always return `"Runner"`:

```rust
/// Derive the iOS process name for native log filtering.
///
/// iOS Flutter apps always use "Runner" as the Xcode target/process name.
/// Unlike macOS, the process name does not correspond to the bundle ID.
fn derive_ios_process_name(_app_id: &Option<String>) -> String {
    "Runner".to_string()
}
```

#### Test Updates

Update existing tests in the same file:

1. `test_derive_ios_process_name_from_bundle_id` (line 429): Currently asserts `derive_ios_process_name(&Some("com.example.myApp"))` returns `"myApp"`. Change assertion to `"Runner"`.

2. `test_derive_ios_process_name_fallback` (line 439): Currently asserts `derive_ios_process_name(&None)` returns `"Runner"`. This test already passes — no change needed but update the test name/comment to clarify the behavior is unconditional.

3. Add a new test confirming the process name is always `"Runner"` regardless of bundle ID:

```rust
#[test]
fn test_derive_ios_process_name_always_runner() {
    // iOS Flutter apps always use "Runner" regardless of bundle ID
    assert_eq!(derive_ios_process_name(&Some("com.example.myApp".to_string())), "Runner");
    assert_eq!(derive_ios_process_name(&Some("org.flutter.app".to_string())), "Runner");
    assert_eq!(derive_ios_process_name(&None), "Runner");
}
```

### Acceptance Criteria

1. `derive_ios_process_name` returns `"Runner"` for any input (including `Some("com.example.myApp")` and `None`)
2. Updated tests pass: `cargo test -p fdemon-app -- derive_ios_process_name`
3. The `IosLogConfig` constructed for simulator/physical paths always has `process_name: "Runner"`
4. `cargo clippy -p fdemon-app -- -D warnings` passes

### Notes

- This is the **root cause** of the reported issue: "no native logs on iOS Simulator". Once fixed, the `xcrun simctl spawn <UDID> log stream --predicate 'process == "Runner"'` command will match the actual Flutter process.
- The existing tests for `build_simctl_log_stream_command` and `build_idevicesyslog_command` in `ios.rs` already use `process_name: "Runner"` in their test configs — they pass and don't need changes.
- The `_app_id` parameter is kept (prefixed with `_`) for API consistency with `derive_macos_process_name` and `derive_android_process_name`, even though iOS ignores it.

---

## Completion Summary

**Status:** Not Started
