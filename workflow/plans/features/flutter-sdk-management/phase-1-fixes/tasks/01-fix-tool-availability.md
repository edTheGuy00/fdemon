## Task: Fix ToolAvailabilityChecked Handler Overwriting Flutter SDK Fields

**Objective**: Fix the critical bug where the `ToolAvailabilityChecked` message handler performs a wholesale `state.tool_availability = availability;` replacement, erasing the `flutter_sdk` and `flutter_sdk_source` fields that `Engine::new()` set moments earlier.

**Depends on**: None

**Severity**: CRITICAL — runtime correctness bug on every TUI startup

### Scope

- `crates/fdemon-app/src/handler/update.rs`: Fix the `ToolAvailabilityChecked` handler to preserve flutter_sdk fields
- `crates/fdemon-app/src/handler/tests.rs`: Add test verifying preservation

### Details

#### The Bug

**File:** `crates/fdemon-app/src/handler/update.rs`, line 1178

```rust
Message::ToolAvailabilityChecked { availability } => {
    state.tool_availability = availability;  // <-- BUG: wholesale replacement
    // ...
}
```

`ToolAvailability::check()` is async and hardcodes `flutter_sdk: false` and `flutter_sdk_source: None` at `crates/fdemon-daemon/src/tool_availability.rs` lines 78-79 (with a comment: "Flutter SDK fields are populated externally by Engine::new()"). When the async check completes and delivers `ToolAvailabilityChecked`, the handler overwrites the correctly-set values.

**Timing:** `Engine::new()` sets the fields synchronously (engine.rs lines 227-230). `spawn_tool_availability_check()` is called immediately after at `crates/fdemon-tui/src/runner.rs` lines 42 and 122. The async check completes within seconds, delivering the overwriting message.

**Current impact:** No code currently reads `tool_availability.flutter_sdk` or `flutter_sdk_source` (all consumers read `state.resolved_sdk` instead), but these fields are intended for UI display in Phase 2. The `state.resolved_sdk` field is unaffected — session spawning still works.

#### The Fix

In the `ToolAvailabilityChecked` handler, preserve the flutter_sdk fields across the struct replacement:

```rust
Message::ToolAvailabilityChecked { availability } => {
    // Preserve Flutter SDK fields — they are set by Engine::new() via
    // synchronous SDK resolution, not by the async ToolAvailability::check().
    let flutter_sdk = state.tool_availability.flutter_sdk;
    let flutter_sdk_source = state.tool_availability.flutter_sdk_source.clone();
    state.tool_availability = availability;
    state.tool_availability.flutter_sdk = flutter_sdk;
    state.tool_availability.flutter_sdk_source = flutter_sdk_source;

    // ... rest of handler unchanged ...
}
```

#### Why Not Fix in ToolAvailability::check()?

The `check()` method lives in `fdemon-daemon` which has no access to `fdemon-app`'s state or the SDK locator result. The two-phase initialization (OS tool probing is async, SDK detection is synchronous) is an intentional design — the fix belongs at the merge point in the handler.

### Acceptance Criteria

1. `ToolAvailabilityChecked` handler preserves `flutter_sdk` and `flutter_sdk_source` from existing state
2. A test verifies that processing `ToolAvailabilityChecked` when `state.resolved_sdk` is `Some(...)` results in `state.tool_availability.flutter_sdk == true` and `flutter_sdk_source.is_some()`
3. A test verifies that processing `ToolAvailabilityChecked` when `state.resolved_sdk` is `None` results in `state.tool_availability.flutter_sdk == false`
4. Existing `ToolAvailabilityChecked` test behavior is preserved (bootable device discovery still triggers when `xcrun_simctl || android_emulator`)
5. `cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

### Testing

```rust
#[test]
fn test_tool_availability_checked_preserves_flutter_sdk_fields() {
    let mut state = AppState::new();
    // Pre-populate SDK state (simulating Engine::new() initialization)
    state.resolved_sdk = Some(fdemon_daemon::test_utils::fake_flutter_sdk());
    state.tool_availability.flutter_sdk = true;
    state.tool_availability.flutter_sdk_source = Some("FVM (3.19.0)".to_string());

    // Simulate the async check result (flutter_sdk fields are false/None)
    let availability = ToolAvailability {
        xcrun_simctl: true,
        ..Default::default()
    };

    let _result = update(&mut state, Message::ToolAvailabilityChecked { availability });

    // Flutter SDK fields must survive the replacement
    assert!(state.tool_availability.flutter_sdk);
    assert_eq!(
        state.tool_availability.flutter_sdk_source.as_deref(),
        Some("FVM (3.19.0)")
    );
    // OS tool fields should be updated
    assert!(state.tool_availability.xcrun_simctl);
}

#[test]
fn test_tool_availability_checked_no_sdk_keeps_false() {
    let mut state = AppState::new();
    // No SDK resolved
    assert!(!state.tool_availability.flutter_sdk);

    let availability = ToolAvailability {
        xcrun_simctl: true,
        ..Default::default()
    };

    let _result = update(&mut state, Message::ToolAvailabilityChecked { availability });

    // Should remain false since no SDK was resolved
    assert!(!state.tool_availability.flutter_sdk);
    assert!(state.tool_availability.flutter_sdk_source.is_none());
}
```

### Notes

- This is a blocking fix — must be resolved before the branch can be merged.
- Headless mode (`src/headless/runner.rs`) does not call `spawn_tool_availability_check()`, so it is unaffected.
- The `spawn_tool_availability_check` timeout path (spawn.rs line 325) also falls back to `ToolAvailability::default()` which has the same `false`/`None` values — the fix in the handler covers both the normal and timeout paths.

---

## Completion Summary

**Status:** Not Started
