## Task: Prevent Bootable Device Discovery from Clearing SDK Errors

**Objective**: Fix `set_bootable_devices()` so it does not unconditionally clear `target_selector.error`, which silently erases SDK-level errors when bootable device discovery completes.

**Depends on**: 01-restore-path-fallback

### Scope

- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`: Remove unconditional `self.error = None` from `set_bootable_devices()`

### Details

#### The Problem

Both `set_connected_devices()` (line 215) and `set_bootable_devices()` (line 237) in `target_selector_state.rs` unconditionally set `self.error = None`. This is correct for `set_connected_devices()` — successful device discovery implies a working SDK, so any prior error is resolved. But `set_bootable_devices()` runs independently of the Flutter SDK (uses `xcrun simctl list` and `emulator -list-avds`), so it should NOT clear SDK-related errors.

#### The Startup Race

```
1. DeviceDiscoveryFailed { error: "Flutter SDK not found..." }
   → target_selector.set_error(...)  →  error = Some("Flutter SDK not found...")

2. ToolAvailabilityChecked → triggers DiscoverBootableDevices

3. BootableDevicesDiscovered { ios_simulators, android_avds }
   → target_selector.set_bootable_devices(...)  →  error = None  (BUG)
```

#### The Fix

Remove `self.error = None;` from `set_bootable_devices()`. Keep it in `set_connected_devices()`.

```rust
// set_bootable_devices — REMOVE self.error = None
pub fn set_bootable_devices(&mut self, ios_simulators: Vec<...>, android_avds: Vec<...>) {
    self.ios_simulators = ios_simulators;
    self.android_avds = android_avds;
    self.bootable_loading = false;
    // self.error = None;  ← REMOVE THIS LINE
    self.invalidate_cache();
    self.scroll_offset = 0;
    // ...
}

// set_connected_devices — KEEP self.error = None (successful discovery clears errors)
pub fn set_connected_devices(&mut self, devices: Vec<Device>) {
    self.connected_devices = devices;
    self.loading = false;
    self.error = None;  // ← KEEP — successful device discovery means SDK works
    self.invalidate_cache();
    self.scroll_offset = 0;
    // ...
}
```

#### Why This Is Safe

- `set_bootable_devices` is called from `BootableDevicesDiscovered` handler, which runs after `xcrun simctl list` / `emulator -list-avds` complete. These tools don't require the Flutter SDK.
- The only errors currently set via `set_error()` come from `DeviceDiscoveryFailed { is_background: false }`, which is SDK-related. There is no "bootable discovery failed" error that flows through `set_error()`.
- If a genuine "bootable discovery failed" error type is needed in the future, it should use a separate field or a typed error enum — but that's out of scope for this fix.

### Acceptance Criteria

1. `set_bootable_devices()` does NOT set `self.error = None`
2. `set_connected_devices()` continues to set `self.error = None` (unchanged)
3. When Flutter SDK is genuinely not found, the "Flutter SDK not found" error persists after bootable device discovery completes
4. When Flutter SDK IS found and device discovery succeeds, `set_connected_devices` clears any prior errors (existing behavior preserved)
5. `cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

### Testing

```rust
#[test]
fn test_set_bootable_devices_does_not_clear_error() {
    let mut state = TargetSelectorState::default();
    state.set_error("Flutter SDK not found".to_string());
    assert!(state.error.is_some());

    state.set_bootable_devices(vec![], vec![]);

    // Error should persist — bootable discovery is independent of SDK
    assert!(state.error.is_some());
    assert_eq!(state.error.as_deref(), Some("Flutter SDK not found"));
    assert!(!state.bootable_loading);
}

#[test]
fn test_set_connected_devices_clears_error() {
    let mut state = TargetSelectorState::default();
    state.set_error("Flutter SDK not found".to_string());
    assert!(state.error.is_some());

    state.set_connected_devices(vec![]);

    // Error should be cleared — successful device discovery means SDK is working
    assert!(state.error.is_none());
    assert!(!state.loading);
}
```

### Notes

- This is a defense-in-depth fix. With Task 01's PATH fallback restored, the "Flutter SDK not found" error should rarely appear on machines with a working Flutter installation. But when it does (genuinely absent SDK), it must persist.
- The `bootable_loading` field is correctly set to `false` by `set_bootable_devices` — that behavior is unchanged.
- If `set_bootable_devices` needs its own error handling in the future (e.g., "xcrun not found"), it should use a separate `bootable_error: Option<String>` field rather than sharing the main `error` field.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | Removed `self.error = None;` from `set_bootable_devices()`, replacing it with a comment explaining the rationale; added two new unit tests |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | Updated existing `test_set_bootable_devices_clears_error` test (which tested the old incorrect behavior) to `test_set_bootable_devices_does_not_clear_error` — asserting that errors persist |

### Notable Decisions/Tradeoffs

1. **Pre-existing contradictory test in fdemon-tui**: The `fdemon-tui` crate had its own test (`test_set_bootable_devices_clears_error`) that asserted the now-incorrect behavior. This test was renamed and updated to match the fix rather than deleted, preserving test coverage of the method while accurately reflecting the correct semantics.

2. **Comment instead of silence**: Added an explanatory comment in `set_bootable_devices()` at the removal site to make it clear this was a deliberate design choice, not an omission, so future maintainers don't accidentally re-add the line.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (all tests across all crates, 0 failures)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo test -p fdemon-app -- new_session_dialog::target_selector_state::tests` - Passed (6/6 tests including both new tests)

### Risks/Limitations

1. **No "bootable discovery failed" path**: As noted in the task, if `set_bootable_devices` needs its own error reporting in the future (e.g., "xcrun not found"), it should use a separate `bootable_error` field rather than sharing `error`. This is out of scope for this fix.
