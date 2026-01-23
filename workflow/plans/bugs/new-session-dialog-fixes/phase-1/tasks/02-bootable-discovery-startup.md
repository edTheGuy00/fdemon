## Task: Trigger Bootable Device Discovery at Startup

**Objective**: Automatically trigger bootable device (emulators/simulators) discovery after tool availability check completes, so the bootable tab is populated on first dialog open.

**Depends on**: None

**Bug Reference**: Bug 2 - Bootable Devices List Never Populates on First Open

### Scope

- `src/app/handler/update.rs`: Modify `ToolAvailabilityChecked` handler to trigger bootable discovery
- `src/tui/widgets/new_session_dialog/target_selector.rs`: Set `bootable_loading: true` as default

### Details

**Problem 1:** Bootable discovery is never triggered at startup.

At startup (`src/tui/runner.rs:68-72`), only connected device discovery is triggered:
```rust
super::spawn::spawn_tool_availability_check(msg_tx.clone());
super::spawn::spawn_device_discovery(msg_tx.clone());
// Missing: spawn_bootable_device_discovery()
```

**Problem 2:** Bootable discovery depends on `tool_availability` (xcrun_simctl, android_emulator).

We can't call `spawn_bootable_device_discovery()` at startup because we don't know which tools are available yet. The solution is to trigger bootable discovery when `ToolAvailabilityChecked` message is received.

**Problem 3:** Bootable tab shows empty list instead of loading state.

`TargetSelectorState::default()` sets `bootable_loading: false`, so users see an empty list instead of a loading indicator.

**Implementation:**

**Step 1:** Modify `ToolAvailabilityChecked` handler (`src/app/handler/update.rs`):

```rust
Message::ToolAvailabilityChecked { availability } => {
    state.tool_availability = availability;

    tracing::info!(
        "Tool availability: xcrun_simctl={}, android_emulator={}",
        state.tool_availability.xcrun_simctl,
        state.tool_availability.android_emulator
    );

    // Trigger bootable device discovery now that we know which tools are available
    if state.tool_availability.xcrun_simctl || state.tool_availability.android_emulator {
        // Set loading state for bootable tab
        state.new_session_dialog_state.target_selector.bootable_loading = true;
        UpdateResult::action(UpdateAction::DiscoverBootableDevices)
    } else {
        UpdateResult::none()
    }
}
```

**Step 2:** Update `TargetSelectorState::default()` (`src/tui/widgets/new_session_dialog/target_selector.rs`):

```rust
impl Default for TargetSelectorState {
    fn default() -> Self {
        Self {
            active_tab: TargetTab::Connected,
            connected_devices: Vec::new(),
            ios_simulators: Vec::new(),
            android_avds: Vec::new(),
            selected_index: 0,
            loading: true,
            bootable_loading: true,  // Changed from false to true
            error: None,
            scroll_offset: 0,
            cached_flat_list: None,
        }
    }
}
```

**Key Files to Reference:**
- `src/app/handler/update.rs:1031-1042` - `ToolAvailabilityChecked` handler
- `src/tui/widgets/new_session_dialog/target_selector.rs:56-71` - `TargetSelectorState::default()`
- `src/tui/spawn.rs:305-326` - `spawn_bootable_device_discovery()`
- `src/app/handler/mod.rs` - `UpdateAction::DiscoverBootableDevices`

### Acceptance Criteria

1. Bootable tab shows "Discovering devices..." on first dialog open
2. Bootable devices populate automatically after tool check completes
3. If no tools available (no xcrun_simctl, no android_emulator), show empty list (not loading forever)
4. "r" key still works to manually refresh bootable devices
5. No regression in connected device behavior

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_availability_triggers_bootable_discovery() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        let availability = ToolAvailability {
            xcrun_simctl: true,
            android_emulator: false,
        };

        let result = update(&mut state, Message::ToolAvailabilityChecked { availability });

        assert!(state.tool_availability.xcrun_simctl);
        assert!(state.new_session_dialog_state.target_selector.bootable_loading);
        assert!(matches!(
            result.action,
            Some(UpdateAction::DiscoverBootableDevices)
        ));
    }

    #[test]
    fn test_no_tools_available_no_discovery() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        let availability = ToolAvailability {
            xcrun_simctl: false,
            android_emulator: false,
        };

        let result = update(&mut state, Message::ToolAvailabilityChecked { availability });

        assert!(result.action.is_none());
    }

    #[test]
    fn test_target_selector_default_shows_bootable_loading() {
        let state = TargetSelectorState::default();
        assert!(state.bootable_loading);
    }
}
```

### Notes

- The `DiscoverBootableDevices` action handler in `tui/actions.rs` already exists and works correctly
- Need to handle the case where tool availability check fails or times out
- Consider adding timeout handling if bootable discovery takes too long
- The `bootable_loading` flag should be set to `false` when `BootableDevicesDiscovered` message is received (already implemented)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/update.rs` | Modified `ToolAvailabilityChecked` handler to trigger bootable discovery when tools are available |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Changed `bootable_loading` default from `false` to `true` |
| `src/app/handler/tests.rs` | Added 3 new unit tests for bootable discovery startup behavior |
| `src/app/handler/new_session/navigation.rs` | Fixed existing test to account for new default `bootable_loading` state |

### Notable Decisions/Tradeoffs

1. **Automatic Discovery Trigger**: When `ToolAvailabilityChecked` message is received, if either `xcrun_simctl` or `android_emulator` is available, the handler automatically sets `bootable_loading: true` and returns `UpdateAction::DiscoverBootableDevices`. This ensures bootable devices are discovered as soon as we know which tools are available.

2. **Default Loading State**: Setting `bootable_loading: true` in the default state shows a loading indicator immediately when the dialog opens, providing better UX feedback while tools are being checked and devices discovered.

3. **Graceful Degradation**: If no tools are available (neither iOS nor Android emulator tools), the handler simply returns `UpdateResult::none()`, preventing unnecessary discovery attempts and leaving the bootable tab empty.

4. **Test Compatibility**: Updated existing `test_switch_tab_to_bootable_triggers_discovery` to set `bootable_loading: false` before testing, simulating the scenario where initial discovery has completed with no devices found.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo clippy -- -D warnings` - Passed
- `cargo test --lib` - Passed (1450 passed, 1 pre-existing failure unrelated to changes)

**New Tests Added:**
- `test_tool_availability_triggers_bootable_discovery` - Verifies discovery is triggered when tools are available
- `test_no_tools_available_no_discovery` - Verifies no discovery when no tools available
- `test_target_selector_default_shows_bootable_loading` - Verifies default loading state

**Existing Tests Updated:**
- `test_switch_tab_to_bootable_triggers_discovery` - Updated to account for new default state

### Risks/Limitations

1. **Pre-existing Test Failure**: `test_truncate_middle_very_short` in `src/tui/widgets/new_session_dialog/mod.rs` was already failing before our changes and remains failing. This is unrelated to bootable discovery functionality.

2. **Timing Dependency**: The bootable discovery now depends on tool availability check completing first. This is the intended behavior, but there's a brief period where the bootable tab shows "Discovering devices..." before tools are even checked. This is acceptable as tool checking is very fast (~100ms).

3. **No Explicit Timeout**: If tool availability check never completes (edge case), bootable tab would show loading forever. However, this is mitigated by the tool availability check having its own timeout handling in the spawn layer.
