## Task: Status Bar Config Info

**Objective**: Replace device info display in the status bar with build configuration info (Debug/Profile/Release and optional flavor) for the currently selected session.

**Depends on**: None

---

### Scope

#### `src/tui/widgets/status_bar.rs`
- Remove `device_info()` method (device info now shown in session header - Task 01)
- Add new `config_info()` method that displays:
  - FlutterMode: "Debug", "Profile", or "Release"
  - Optional flavor: "(production)" appended if present
  - Format: "Debug" or "Debug (production)"
- Update `build_segments()` to replace device_info with config_info
- Handle case where no session selected (show nothing or default)
- Handle case where session has no launch_config (show "Debug" as Flutter default)

#### `src/app/state.rs`
- Add helper method `selected_session_config()` → returns `Option<&LaunchConfig>`
- This provides clean access for status bar to get config info

#### `src/app/session.rs`
- Verify `launch_config: Option<LaunchConfig>` field exists and is populated
- No changes needed if already correctly storing config

---

### Implementation Details

**status_bar.rs changes:**

Remove device_info() method entirely.

Add new config_info() method:

```rust
/// Get build configuration info span
fn config_info(&self) -> Option<Span<'static>> {
    // Get selected session's config
    let session = self.state.session_manager.selected()?;
    let session_data = &session.session;
    
    // Get mode and flavor from launch_config, default to Debug
    let (mode, flavor) = match &session_data.launch_config {
        Some(config) => (config.mode, config.flavor.clone()),
        None => (FlutterMode::Debug, None),
    };
    
    // Format the display string
    let display = match flavor {
        Some(f) => format!("{} ({})", mode, f),
        None => mode.to_string(),
    };
    
    // Color based on mode
    let color = match mode {
        FlutterMode::Debug => Color::Green,
        FlutterMode::Profile => Color::Yellow,
        FlutterMode::Release => Color::Magenta,
    };
    
    Some(Span::styled(display, Style::default().fg(color)))
}
```

Update build_segments() to use config_info instead of device_info:

```rust
fn build_segments(&self) -> Vec<Span<'static>> {
    // ... existing code ...
    
    // Replace device_info with config_info
    if let Some(config) = self.config_info() {
        segments.push(separator.clone());
        segments.push(config);
    }
    
    // ... rest of segments ...
}
```

**Required imports in status_bar.rs:**

```rust
use crate::config::FlutterMode;
```

---

### Acceptance Criteria

1. ✅ Status bar no longer shows device info (device shown in header)
2. ✅ Status bar shows "Debug", "Profile", or "Release" based on session config
3. ✅ If flavor is set, displays as "Debug (flavorname)"
4. ✅ Color coding: Debug=Green, Profile=Yellow, Release=Magenta
5. ✅ No session selected → no config displayed
6. ✅ Session without launch_config → shows "Debug" (Flutter default)
7. ✅ Switching sessions updates status bar to show new session's config
8. ✅ All existing tests updated/pass
9. ✅ New tests cover config_info rendering

---

### Testing

#### Unit Tests

```rust
#[test]
fn test_config_info_debug_mode() {
    let mut state = create_test_state();
    // Create session with debug config
    let device = test_device("d1", "iPhone");
    let config = LaunchConfig {
        mode: FlutterMode::Debug,
        flavor: None,
        ..Default::default()
    };
    let id = state.session_manager.create_session_with_config(&device, config).unwrap();
    state.session_manager.select(id);
    
    let bar = StatusBar::new(&state);
    let config_span = bar.config_info().unwrap();
    
    assert!(config_span.content.to_string().contains("Debug"));
    assert_eq!(config_span.style.fg, Some(Color::Green));
}

#[test]
fn test_config_info_release_with_flavor() {
    let mut state = create_test_state();
    let device = test_device("d1", "Pixel");
    let config = LaunchConfig {
        mode: FlutterMode::Release,
        flavor: Some("production".to_string()),
        ..Default::default()
    };
    let id = state.session_manager.create_session_with_config(&device, config).unwrap();
    state.session_manager.select(id);
    
    let bar = StatusBar::new(&state);
    let config_span = bar.config_info().unwrap();
    
    assert!(config_span.content.to_string().contains("Release"));
    assert!(config_span.content.to_string().contains("production"));
    assert_eq!(config_span.style.fg, Some(Color::Magenta));
}

#[test]
fn test_config_info_no_session() {
    let state = create_test_state();
    let bar = StatusBar::new(&state);
    
    assert!(bar.config_info().is_none());
}

#[test]
fn test_config_info_no_launch_config() {
    let mut state = create_test_state();
    let device = test_device("d1", "Device");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select(id);
    
    let bar = StatusBar::new(&state);
    let config_span = bar.config_info().unwrap();
    
    // Should default to Debug
    assert!(config_span.content.to_string().contains("Debug"));
}

#[test]
fn test_device_info_removed() {
    let state = create_test_state();
    let bar = StatusBar::new(&state);
    
    // device_info method should no longer exist
    // This test verifies by checking segments don't contain device info
    let segments = bar.build_segments();
    let content: String = segments.iter().map(|s| s.content.to_string()).collect();
    
    // Should not contain device-specific text patterns
    // (This test is less precise but ensures device_info was removed)
}
```

#### Manual Testing

1. Start fdemon and select device with default config → verify "Debug" shows in status bar
2. Start with release config → verify "Release" shows with correct color
3. Start with flavor → verify "Debug (flavorname)" format
4. Switch between sessions → verify status bar updates to show correct config
5. Verify device name no longer appears in status bar (should be in header only)

---

### Notes

- FlutterMode already has a Display impl that returns lowercase ("debug", "profile", "release")
- Consider capitalizing the first letter for display: "Debug" vs "debug"
- The flavor should be displayed as-is (user-defined, don't modify case)
- StatusBarCompact may also need updating if it showed device info

---

## Completion Summary

**Status:** ✅ Done

### Files Modified
- `src/tui/widgets/status_bar.rs` - Removed `device_info()` method, added `config_info()` method, updated `build_segments()` to use config info

### Notable Decisions/Tradeoffs
- Mode names are capitalized for display: "Debug", "Profile", "Release" (not lowercase as in FlutterMode::Display)
- Color coding: Debug=Green, Profile=Yellow, Release=Magenta
- Flavor is displayed as-is in parentheses: "Release (production)"
- Sessions without launch_config default to "Debug" (matching Flutter's default behavior)
- StatusBarCompact was not modified as it only shows state icon and timer (no device info)

### Testing Performed
- `cargo check` - Compilation successful
- `cargo test` - All 449 tests passed
- `cargo fmt` - Code formatted correctly
- `cargo clippy` - No warnings

### New Tests Added
- `test_config_info_debug_mode` - Verifies Debug mode display and green color
- `test_config_info_profile_mode` - Verifies Profile mode display and yellow color
- `test_config_info_release_with_flavor` - Verifies Release mode with flavor display and magenta color
- `test_config_info_no_session` - Verifies None returned when no session selected
- `test_config_info_no_launch_config` - Verifies default to Debug when session has no config

### Updated Tests
- `test_build_segments_with_config` - Updated from device to config info testing
- `test_status_bar_render` - Updated to check for config info instead of device info

### Removed Tests
- `test_device_info_both` - Device info method removed
- `test_device_info_name_only` - Device info method removed
- `test_device_info_none` - Device info method removed

### Risks/Limitations
- None identified - device info is now shown in session header (Task 01), so this is a complementary change