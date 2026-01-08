## Task: Add DeviceSelector Widget Tests

**Objective**: Add TestBackend-based unit tests for the DeviceSelector widget to verify device list rendering, selection highlighting, and navigation states.

**Depends on**: 06-testbackend-utilities

### Scope

- `src/tui/widgets/device_selector.rs`: Add inline test module

### Details

#### 1. Review DeviceSelector Widget

The DeviceSelector displays:
- Modal overlay with device list
- Selected device highlighting
- Device type icons/indicators
- "No devices" empty state
- Navigation hints (arrows, Enter, Escape)

#### 2. Add Test Module

Add to `src/tui/widgets/device_selector.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_utils::TestTerminal;
    use crate::daemon::devices::Device;

    fn create_mock_device(id: &str, name: &str) -> Device {
        Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "linux".to_string(),
            ..Default::default()
        }
    }

    fn create_selector_state_with_devices(devices: Vec<Device>) -> DeviceSelectorState {
        let mut state = DeviceSelectorState::new();
        for device in devices {
            state.add_device(device);
        }
        state
    }

    #[test]
    fn test_device_selector_renders_title() {
        let mut term = TestTerminal::new();
        let state = DeviceSelectorState::new();
        let selector = DeviceSelector::with_session_state(&state, false);

        term.render_widget(selector, term.area());

        assert!(
            term.buffer_contains("Select") || term.buffer_contains("Device"),
            "Should show device selector title"
        );
    }

    #[test]
    fn test_device_selector_shows_device_list() {
        let mut term = TestTerminal::new();
        let devices = vec![
            create_mock_device("linux", "Linux Desktop"),
            create_mock_device("chrome", "Chrome"),
        ];
        let state = create_selector_state_with_devices(devices);
        let selector = DeviceSelector::with_session_state(&state, false);

        term.render_widget(selector, term.area());

        assert!(term.buffer_contains("Linux"), "Should show Linux device");
        assert!(term.buffer_contains("Chrome"), "Should show Chrome device");
    }

    #[test]
    fn test_device_selector_highlights_selected() {
        let mut term = TestTerminal::new();
        let devices = vec![
            create_mock_device("linux", "Linux Desktop"),
            create_mock_device("chrome", "Chrome"),
        ];
        let mut state = create_selector_state_with_devices(devices);
        state.select_index(1); // Select Chrome

        let selector = DeviceSelector::with_session_state(&state, false);
        term.render_widget(selector, term.area());

        // Both should appear, Chrome should be highlighted (verify visually or by position)
        assert!(term.buffer_contains("Chrome"));
    }

    #[test]
    fn test_device_selector_empty_state() {
        let mut term = TestTerminal::new();
        let state = DeviceSelectorState::new(); // No devices
        let selector = DeviceSelector::with_session_state(&state, false);

        term.render_widget(selector, term.area());

        assert!(
            term.buffer_contains("No device") || term.buffer_contains("no device"),
            "Should show empty state message"
        );
    }

    #[test]
    fn test_device_selector_shows_hints() {
        let mut term = TestTerminal::new();
        let devices = vec![create_mock_device("linux", "Linux")];
        let state = create_selector_state_with_devices(devices);
        let selector = DeviceSelector::with_session_state(&state, false);

        term.render_widget(selector, term.area());

        // Should show navigation hints
        assert!(
            term.buffer_contains("Enter") ||
            term.buffer_contains("↑") ||
            term.buffer_contains("↓") ||
            term.buffer_contains("Esc"),
            "Should show navigation hints"
        );
    }

    #[test]
    fn test_device_selector_with_running_sessions() {
        let mut term = TestTerminal::new();
        let state = DeviceSelectorState::new();
        // has_sessions = true should show different UI (e.g., "Add device" vs "Select device")
        let selector = DeviceSelector::with_session_state(&state, true);

        term.render_widget(selector, term.area());

        // Should render with session-aware messaging
        let content = term.content();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_device_selector_modal_overlay() {
        let mut term = TestTerminal::new();
        let state = DeviceSelectorState::new();
        let selector = DeviceSelector::with_session_state(&state, false);

        term.render_widget(selector, term.area());

        // Modal should have borders or clear background
        // This is more of a visual test - just verify it renders
        let content = term.content();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_device_selector_many_devices() {
        let mut term = TestTerminal::new();
        let devices: Vec<Device> = (0..10)
            .map(|i| create_mock_device(&format!("device{}", i), &format!("Device {}", i)))
            .collect();
        let state = create_selector_state_with_devices(devices);
        let selector = DeviceSelector::with_session_state(&state, false);

        term.render_widget(selector, term.area());

        // Should handle many devices (may need scrolling)
        assert!(term.buffer_contains("Device 0"));
    }

    #[test]
    fn test_device_selector_compact() {
        let mut term = TestTerminal::compact();
        let devices = vec![create_mock_device("linux", "Linux")];
        let state = create_selector_state_with_devices(devices);
        let selector = DeviceSelector::with_session_state(&state, false);

        term.render_widget(selector, term.area());

        // Should fit in compact terminal
        let content = term.content();
        assert!(!content.is_empty());
    }
}
```

### Test Coverage

| Test Case | Verifies |
|-----------|----------|
| `test_device_selector_renders_title` | Title appears |
| `test_device_selector_shows_device_list` | Devices listed |
| `test_device_selector_highlights_selected` | Selection visible |
| `test_device_selector_empty_state` | "No devices" message |
| `test_device_selector_shows_hints` | Navigation hints |
| `test_device_selector_with_running_sessions` | Session-aware UI |
| `test_device_selector_modal_overlay` | Modal renders |
| `test_device_selector_many_devices` | Handles long list |
| `test_device_selector_compact` | Works in small terminal |

### Acceptance Criteria

1. Device list renders correctly
2. Selection highlighting works
3. Empty state handled gracefully
4. Navigation hints displayed
5. Works in various terminal sizes

### Testing

```bash
# Run device selector tests
cargo test widgets::device_selector --lib -- --nocapture
```

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/device_selector.rs` | Added 9 TestTerminal-based widget tests (lines 1107-1285) |

### Notable Decisions/Tradeoffs

1. **Helper Functions**: Created `create_mock_device()` and `create_selector_state_with_devices()` helper functions specific to the TestTerminal tests to avoid conflicts with existing test helpers that use different patterns.

2. **Empty State Test**: Modified the empty state test to disable emulator options (`state.show_emulator_options = false`) to achieve a true empty state, since the DeviceSelector shows emulator launch options by default even when no devices are present.

3. **Selection Test**: Used `state.select_next()` instead of `state.select_index(1)` as specified in the task plan, since `select_next()` is the public API method available on `DeviceSelectorState`.

### Testing Performed

- `cargo test widgets::device_selector --lib -- --nocapture` - Passed (44 tests total, including 9 new tests)
- `cargo test --lib` - Passed (1300 tests)
- `cargo fmt --check` - Passed
- `cargo clippy --lib -- -D warnings` - Passed

### Risks/Limitations

1. **Visual Testing**: Some tests (like `test_device_selector_highlights_selected`) verify that content renders but cannot directly verify visual highlighting due to the limitations of buffer-based testing. These tests confirm the content appears but not the exact styling.

2. **Existing Tests**: The file already had comprehensive tests using `TestBackend` directly. The new tests demonstrate the TestTerminal API as requested but provide similar coverage to existing tests. This is intentional to showcase the new test utility pattern.
