## Task: Device Discovery Caching with Refresh Indicator

**Objective**: Cache discovered devices to provide instant display on subsequent device selector opens, while showing a refresh indicator (LineGauge in header) during background refresh.

**Depends on**: 11-linegauge-progress.md (LineGauge implementation)

---

### Scope

- `src/tui/widgets/device_selector.rs`: Add cache state, refreshing mode, header LineGauge
- `src/app/handler.rs`: Update ShowDeviceSelector to use cache
- `src/app/state.rs`: Consider adding device cache at AppState level (alternative)

---

### Problem Statement

Currently, every time the device selector is shown, it:
1. Sets `loading = true`
2. Triggers `DiscoverDevices` action
3. Shows only a loading indicator until discovery completes

The `flutter devices` command can take 2-5+ seconds. This creates poor UX when:
- User presses `n` to add a new session
- User changes their mind and wants to see available devices quickly
- User accidentally closes the selector and reopens it

### Desired Behavior

| Scenario | Current Behavior | New Behavior |
|----------|-----------------|--------------|
| **Startup** | LineGauge only, wait for discovery | LineGauge only, wait for discovery (same) |
| **Subsequent open** | LineGauge only, wait for discovery | Show cached devices immediately, LineGauge at header indicates refresh |
| **After refresh completes** | Show devices | Update device list (may add/remove devices) |

---

### Implementation Details

#### 1. Add Cache and Refreshing State to DeviceSelectorState

```rust
#[derive(Debug, Clone, Default)]
pub struct DeviceSelectorState {
    /// Available devices (current view)
    pub devices: Vec<Device>,

    /// Cached devices from last successful discovery
    /// Used for instant display on subsequent opens
    cached_devices: Option<Vec<Device>>,

    /// Whether we're doing initial load (no cache, show centered LineGauge)
    pub loading: bool,

    /// Whether we're refreshing in background (has cache, show header LineGauge)
    pub refreshing: bool,

    // ... existing fields ...
}

impl DeviceSelectorState {
    /// Show loading state (startup, no cache)
    pub fn show_loading(&mut self) {
        self.visible = true;
        self.loading = true;
        self.refreshing = false;
        self.error = None;
    }

    /// Show with cached devices, refresh in background
    pub fn show_refreshing(&mut self) {
        self.visible = true;
        
        // Use cached devices if available
        if let Some(ref cached) = self.cached_devices {
            self.devices = cached.clone();
            self.loading = false;
            self.refreshing = true;
        } else {
            // No cache, fall back to loading
            self.loading = true;
            self.refreshing = false;
        }
        self.error = None;
    }

    /// Check if we have cached devices
    pub fn has_cache(&self) -> bool {
        self.cached_devices.is_some()
    }

    /// Set devices after discovery (updates cache)
    pub fn set_devices(&mut self, devices: Vec<Device>) {
        self.devices = devices.clone();
        self.cached_devices = Some(devices);
        self.loading = false;
        self.refreshing = false;
    }
    
    /// Clear cache (e.g., after error or explicit refresh request)
    pub fn clear_cache(&mut self) {
        self.cached_devices = None;
    }
}
```

#### 2. Update ShowDeviceSelector Handler

```rust
// In src/app/handler.rs
Message::ShowDeviceSelector => {
    state.ui_mode = UiMode::DeviceSelector;
    
    // Use cache if available, otherwise full loading
    if state.device_selector.has_cache() {
        state.device_selector.show_refreshing();
    } else {
        state.device_selector.show_loading();
    }
    
    // Always trigger discovery to get fresh data
    UpdateResult::action(UpdateAction::DiscoverDevices)
}
```

#### 3. Update DevicesDiscovered Handler

```rust
Message::DevicesDiscovered { devices } => {
    let device_count = devices.len();
    
    // set_devices now also updates cache
    state.device_selector.set_devices(devices);
    
    // ... rest of logging ...
    UpdateResult::none()
}
```

#### 4. Update Widget Rendering for Refreshing State

The device selector needs to handle three states:
1. **Loading** (`loading=true, refreshing=false`): Show centered LineGauge only
2. **Refreshing** (`loading=false, refreshing=true`): Show header LineGauge + cached device list
3. **Ready** (`loading=false, refreshing=false`): Show device list only

```rust
impl Widget for DeviceSelector<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal_area = Self::centered_rect(60, 70, area);
        Clear.render(modal_area, buf);

        let block = Block::default()
            .title(" Select Target Device ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .style(Style::default().bg(Color::DarkGray));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        // Determine layout based on state
        let (content_area, footer_area) = if self.state.refreshing {
            // Refreshing: header gauge + content + footer
            let chunks = Layout::vertical([
                Constraint::Length(1), // Refresh indicator
                Constraint::Min(3),    // Content (cached devices)
                Constraint::Length(2), // Footer
            ])
            .split(inner);
            
            // Render refresh indicator in header
            self.render_refresh_indicator(chunks[0], buf);
            
            (chunks[1], chunks[2])
        } else {
            // Normal or loading: content + footer
            let chunks = Layout::vertical([
                Constraint::Min(3),    // Content
                Constraint::Length(2), // Footer
            ])
            .split(inner);
            
            (chunks[0], chunks[1])
        };

        // Render content based on state
        if self.state.loading {
            self.render_loading_state(content_area, buf);
        } else if let Some(ref error) = self.state.error {
            self.render_error_state(error, content_area, buf);
        } else if self.state.is_empty() {
            self.render_empty_state(content_area, buf);
        } else {
            self.render_device_list(content_area, buf);
        }

        // Footer
        self.render_footer(footer_area, buf);
    }
}
```

#### 5. Render Refresh Indicator (Header LineGauge)

```rust
impl DeviceSelector<'_> {
    fn render_refresh_indicator(&self, area: Rect, buf: &mut Buffer) {
        // Compact horizontal LineGauge showing refresh in progress
        let ratio = self.state.indeterminate_ratio();
        
        // Pad the gauge area
        let gauge_area = Rect {
            x: area.x + 2,
            y: area.y,
            width: area.width.saturating_sub(4),
            height: 1,
        };

        let gauge = LineGauge::default()
            .ratio(ratio)
            .filled_style(Style::default().fg(Color::Yellow))  // Yellow for refresh
            .unfilled_style(Style::default().fg(Color::Black))
            .line_set(symbols::line::NORMAL);  // Thinner for header

        gauge.render(gauge_area, buf);
    }
}
```

---

### Visual Design

#### State: Loading (No Cache)
```
┌─────────────────────────────────────────────┐
│           Select Target Device              │
├─────────────────────────────────────────────┤
│                                             │
│          Discovering devices...             │
│                                             │
│      ────────━━━━━━━━━━━────────           │
│                                             │
│                                             │
├─────────────────────────────────────────────┤
│      ↑↓ Navigate  Enter Select  r Refresh  │
└─────────────────────────────────────────────┘
```

#### State: Refreshing (With Cache)
```
┌─────────────────────────────────────────────┐
│           Select Target Device              │
├─────────────────────────────────────────────┤
│  ─────────━━━━━━━━━━━───────────           │  <-- Yellow refresh indicator
│▶ iPhone 15 Pro                   (physical) │
│  Pixel 7 API 34                 (emulator)  │
│  macOS                           (desktop)  │
│  ─────────────────────────────────────────  │
│  + Launch Android Emulator...               │
│  + Launch iOS Simulator...                  │
├─────────────────────────────────────────────┤
│  ↑↓ Navigate  Enter Select  Esc Cancel  r  │
└─────────────────────────────────────────────┘
```

#### State: Ready (Cache Updated)
```
┌─────────────────────────────────────────────┐
│           Select Target Device              │
├─────────────────────────────────────────────┤
│▶ iPhone 15 Pro                   (physical) │
│  Pixel 7 API 34                 (emulator)  │
│  macOS                           (desktop)  │
│  Chrome                              (web)  │  <-- New device found
│  ─────────────────────────────────────────  │
│  + Launch Android Emulator...               │
│  + Launch iOS Simulator...                  │
├─────────────────────────────────────────────┤
│  ↑↓ Navigate  Enter Select  Esc Cancel  r  │
└─────────────────────────────────────────────┘
```

---

### Edge Cases

| Scenario | Handling |
|----------|----------|
| Device removed while cached | Refresh updates list; selecting removed device would fail |
| Discovery fails during refresh | Keep cached devices, show error briefly, allow retry with 'r' |
| User selects device during refresh | Selection works immediately; device might disappear after refresh |
| Cache very stale (hours old) | Consider adding cache TTL in future |
| 'r' key during refresh | Ignore or restart refresh |

---

### Acceptance Criteria

1. [ ] `DeviceSelectorState` has `cached_devices` field
2. [ ] `DeviceSelectorState` has `refreshing` boolean
3. [ ] `show_loading()` sets loading=true, refreshing=false
4. [ ] `show_refreshing()` uses cache if available, sets refreshing=true
5. [ ] `set_devices()` updates both devices and cache
6. [ ] `has_cache()` returns true after first successful discovery
7. [ ] ShowDeviceSelector uses `show_refreshing()` when cache exists
8. [ ] Refreshing state shows header LineGauge (yellow)
9. [ ] Refreshing state shows cached device list below indicator
10. [ ] Loading state shows centered LineGauge (cyan)
11. [ ] Device list updates when discovery completes during refresh
12. [ ] Footer remains visible in all states

---

### Testing

```rust
#[test]
fn test_initial_show_loading_no_cache() {
    let mut state = DeviceSelectorState::new();
    assert!(!state.has_cache());
    
    state.show_loading();
    
    assert!(state.loading);
    assert!(!state.refreshing);
    assert!(state.devices.is_empty());
}

#[test]
fn test_show_refreshing_with_cache() {
    let mut state = DeviceSelectorState::new();
    
    // First discovery
    let devices = vec![test_device("iphone", "iPhone 15")];
    state.set_devices(devices.clone());
    
    assert!(state.has_cache());
    assert!(!state.loading);
    assert!(!state.refreshing);
    
    // Subsequent show
    state.show_refreshing();
    
    assert!(!state.loading);
    assert!(state.refreshing);
    assert_eq!(state.devices.len(), 1);
}

#[test]
fn test_show_refreshing_falls_back_to_loading() {
    let mut state = DeviceSelectorState::new();
    assert!(!state.has_cache());
    
    // No cache, should fall back to loading
    state.show_refreshing();
    
    assert!(state.loading);
    assert!(!state.refreshing);
}

#[test]
fn test_set_devices_updates_cache() {
    let mut state = DeviceSelectorState::new();
    
    let devices = vec![
        test_device("device1", "Device 1"),
        test_device("device2", "Device 2"),
    ];
    state.set_devices(devices);
    
    assert!(state.has_cache());
    assert_eq!(state.devices.len(), 2);
    
    // Hide and show again
    state.hide();
    state.show_refreshing();
    
    // Should have cached devices
    assert_eq!(state.devices.len(), 2);
}

#[test]
fn test_refresh_updates_device_list() {
    let mut state = DeviceSelectorState::new();
    
    // Initial devices
    state.set_devices(vec![test_device("device1", "Device 1")]);
    state.show_refreshing();
    
    assert!(state.refreshing);
    assert_eq!(state.devices.len(), 1);
    
    // Discovery completes with new devices
    state.set_devices(vec![
        test_device("device1", "Device 1"),
        test_device("device2", "Device 2 (new)"),
    ]);
    
    assert!(!state.refreshing);
    assert_eq!(state.devices.len(), 2);
}

#[test]
fn test_clear_cache() {
    let mut state = DeviceSelectorState::new();
    state.set_devices(vec![test_device("device1", "Device 1")]);
    
    assert!(state.has_cache());
    
    state.clear_cache();
    
    assert!(!state.has_cache());
}

#[test]
fn test_render_refreshing_shows_header_gauge() {
    use ratatui::{backend::TestBackend, Terminal};
    
    let mut state = DeviceSelectorState::new();
    state.set_devices(vec![test_device("iphone", "iPhone 15")]);
    state.show_refreshing();
    
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    
    terminal.draw(|f| {
        let selector = DeviceSelector::new(&state);
        f.render_widget(selector, f.area());
    }).unwrap();
    
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    
    // Should show device name
    assert!(content.contains("iPhone 15"));
    
    // Should have gauge characters (from header indicator)
    assert!(content.contains('━') || content.contains('─'));
}
```

---

### Implementation Notes

1. **Cache Lifetime**: The cache persists for the lifetime of the application. Consider adding TTL if devices can become stale.

2. **Memory Usage**: Caching devices is minimal overhead (Vec of small structs).

3. **Selection Index**: When devices are updated during refresh, the selected index might point to a different device or become invalid. Consider:
   - Preserving selection by device ID rather than index
   - Resetting selection to 0 when device list changes

4. **Color Scheme**:
   - Loading (no cache): Cyan (matches primary accent)
   - Refreshing (with cache): Yellow (indicates background activity)
   - This distinguishes between "waiting to see anything" and "updating what you see"

5. **Animation**: Both loading and refreshing use the same `indeterminate_ratio()` from Task 11.

6. **Future Enhancement**: Could add a timestamp to the cache and show "Last updated: X seconds ago" in the footer.

---

## Completion Summary

**Status:** ✅ Done

### Files Modified

- `src/tui/widgets/device_selector.rs` - Added `cached_devices` and `refreshing` fields, `show_refreshing()`, `has_cache()`, `clear_cache()` methods, updated `set_devices()` to update cache, updated Widget render for refreshing state with header LineGauge, added 9 new tests
- `src/app/handler.rs` - Updated `ShowDeviceSelector` handler to use cache when available, updated `Tick` handler to tick when refreshing, added 2 new tests

### Notable Decisions/Tradeoffs

1. **Cache persistence**: Cache persists for application lifetime (no TTL implemented)
2. **Yellow LineGauge for refresh**: Distinguishes from cyan loading indicator (waiting for anything vs updating existing)
3. **Normal line set for header**: Uses thinner `NORMAL.horizontal` vs `THICK.horizontal` for header gauge
4. **Selection index reset**: `set_devices()` resets selection to 0 when devices change (could be enhanced to preserve by device ID)

### Testing Performed

```
cargo check - PASS (no warnings)
cargo test --lib device_selector - PASS (41 tests)
cargo test --lib - PASS (451 tests)
cargo clippy - PASS (only pre-existing warning in tui/mod.rs:390)
cargo fmt - PASS
```

### New Tests Added

In `device_selector.rs`:
- `test_initial_has_no_cache` - Verifies new state has no cache
- `test_show_loading_no_cache` - Verifies loading mode with no cache
- `test_show_refreshing_with_cache` - Verifies refreshing mode uses cache
- `test_show_refreshing_falls_back_to_loading` - Verifies fallback when no cache
- `test_set_devices_updates_cache` - Verifies cache updated on discovery
- `test_refresh_updates_device_list` - Verifies refresh completes correctly
- `test_clear_cache` - Verifies cache can be cleared
- `test_set_error_clears_refreshing` - Verifies error clears refreshing state
- `test_render_refreshing_shows_header_gauge_and_devices` - Verifies render shows both

In `handler.rs`:
- `test_tick_advances_when_refreshing` - Verifies tick works in refresh mode
- `test_show_device_selector_uses_cache` - Verifies handler uses cache correctly

### Risks/Limitations

- Cache never expires (no TTL) - devices could become stale after hours
- Selection index resets on refresh - could cause unexpected selection changes
- No indication to user of cache age