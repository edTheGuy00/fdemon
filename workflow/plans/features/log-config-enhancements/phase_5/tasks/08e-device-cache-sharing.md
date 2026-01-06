# Task: Device Cache Sharing

**Objective**: Share device cache between DeviceSelector and StartupDialog to avoid redundant discovery.

**Depends on**: Task 08c (Device Discovery Integration)

## Problem

When pressing 'n' to start a new session, device discovery restarts from scratch. The `DeviceSelectorState` has a `cached_devices` field that provides instant display on subsequent opens, but `StartupDialogState` doesn't use this cache.

### Current Behavior

`DeviceSelectorState` (device_selector.rs:34-36):
```rust
/// Cached devices from last successful discovery
/// Used for instant display on subsequent opens
cached_devices: Option<Vec<Device>>,
```

`StartupDialogState` (state.rs:244-261):
```rust
impl Default for StartupDialogState {
    fn default() -> Self {
        Self {
            devices: Vec::new(),  // Always empty on init!
            loading: true,
            // ...
        }
    }
}
```

### Expected Behavior

1. Initial startup: discover devices, cache them globally
2. Press 'n': show cached devices immediately, refresh in background
3. Press 'r': force refresh, update cache

## Scope

- `src/app/state.rs` - Add global device cache to AppState
- `src/app/handler/update.rs` - Update handlers to use shared cache
- `src/tui/startup.rs` - Use cached devices on startup dialog show

## Implementation

### 1. Add Global Device Cache to AppState

```rust
// In state.rs
pub struct AppState {
    // ... existing fields

    /// Global device cache (shared between DeviceSelector and StartupDialog)
    pub device_cache: Option<Vec<Device>>,

    /// When devices were last discovered (for cache invalidation)
    pub devices_last_updated: Option<std::time::Instant>,
}

impl AppState {
    /// Get cached devices (if fresh enough)
    pub fn get_cached_devices(&self) -> Option<&Vec<Device>> {
        // Consider cache valid for 30 seconds
        const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

        if let (Some(ref devices), Some(updated)) = (&self.device_cache, self.devices_last_updated) {
            if updated.elapsed() < CACHE_TTL {
                return Some(devices);
            }
        }
        None
    }

    /// Update device cache
    pub fn set_device_cache(&mut self, devices: Vec<Device>) {
        self.device_cache = Some(devices);
        self.devices_last_updated = Some(std::time::Instant::now());
    }
}
```

### 2. Update ShowStartupDialog Handler

```rust
// In update.rs
Message::ShowStartupDialog => {
    let configs = crate::config::load_all_configs(&state.project_path);
    state.show_startup_dialog(configs);

    // Use cached devices if available
    if let Some(cached) = state.get_cached_devices() {
        state.startup_dialog_state.set_devices(cached.clone());
        state.startup_dialog_state.refreshing = true;
    }

    // Trigger refresh anyway (background update)
    UpdateResult::action(UpdateAction::DiscoverDevices)
}
```

### 3. Update DevicesDiscovered Handler

```rust
Message::DevicesDiscovered { devices } => {
    // Update global cache
    state.set_device_cache(devices.clone());

    // Update device_selector
    state.device_selector.set_devices(devices.clone());

    // Update startup_dialog_state if active
    if state.ui_mode == UiMode::StartupDialog {
        state.startup_dialog_state.set_devices(devices);
    }

    // ... rest unchanged
}
```

### 4. Update show_startup_dialog Method

```rust
impl AppState {
    pub fn show_startup_dialog(&mut self, configs: LoadedConfigs) {
        self.startup_dialog_state = StartupDialogState::with_configs(configs);

        // Pre-populate with cached devices
        if let Some(cached) = self.get_cached_devices() {
            self.startup_dialog_state.devices = cached.clone();
            self.startup_dialog_state.loading = false;
            self.startup_dialog_state.refreshing = true;
            if !cached.is_empty() {
                self.startup_dialog_state.selected_device = Some(0);
            }
        }

        self.ui_mode = UiMode::StartupDialog;
    }
}
```

## Acceptance Criteria

1. First startup: devices discovered and cached
2. Press 'n' for new session: cached devices shown instantly
3. Background refresh updates device list
4. Press 'r': forces refresh, updates cache
5. Cache expires after reasonable TTL (30 seconds suggested)
6. Unit tests for cache behavior

## Testing

```rust
#[test]
fn test_device_cache_shared() {
    let mut state = AppState::new();

    // Simulate initial discovery
    let devices = vec![test_device("dev1", "Device 1")];
    state.set_device_cache(devices.clone());

    // Show startup dialog should use cache
    state.show_startup_dialog(LoadedConfigs::default());

    assert!(!state.startup_dialog_state.loading);
    assert!(state.startup_dialog_state.refreshing);
    assert_eq!(state.startup_dialog_state.devices.len(), 1);
}

#[test]
fn test_device_cache_expires() {
    let mut state = AppState::new();
    state.set_device_cache(vec![test_device("dev1", "Device 1")]);

    // Fresh cache
    assert!(state.get_cached_devices().is_some());

    // Expired cache (mock time travel)
    state.devices_last_updated = Some(std::time::Instant::now() - std::time::Duration::from_secs(60));
    assert!(state.get_cached_devices().is_none());
}
```

## Notes

- Cache TTL of 30 seconds balances freshness vs. responsiveness
- Device list changes are rare (device connects/disconnects)
- Consider emitting a log message when using cached vs fresh devices
- The cache is also useful for DeviceSelector (add-session flow)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/state.rs` | Added `device_cache` and `devices_last_updated` fields to AppState; implemented `get_cached_devices()` and `set_device_cache()` helper methods with 30-second TTL; updated `show_startup_dialog()` to pre-populate with cached devices; added 8 unit tests for cache behavior |
| `src/app/handler/update.rs` | Updated `DevicesDiscovered` handler to update global cache first; modified `ShowDeviceSelector` handler to use global cache for instant display; updated `ShowStartupDialog` handler comment to indicate cache usage |
| `src/app/handler/tests.rs` | Updated `test_show_device_selector_uses_cache` to use global cache instead of local cache |
| `src/tui/startup.rs` | Updated device discovery in `auto_start_session` to cache devices globally after successful discovery (2 locations) |

### Notable Decisions/Tradeoffs

1. **Cache TTL**: Set to 30 seconds to balance freshness with responsiveness. Device list changes (connects/disconnects) are rare, making this a safe tradeoff.
2. **Borrow checker workaround**: Used `.cloned()` to clone cached devices before mutating state to satisfy Rust's borrow checker.
3. **Manual state setting**: In `ShowDeviceSelector` handler, manually set device selector fields instead of using `show_refreshing()` to avoid clearing the refreshing flag after `set_devices()`.
4. **Global cache priority**: Cache is updated in `DevicesDiscovered` handler first, then distributed to DeviceSelector and StartupDialog, ensuring single source of truth.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1187 tests, including 8 new cache tests)
- `cargo clippy` - Passed (no warnings after fixing map_clone suggestions)
- `cargo fmt` - Applied

### Risks/Limitations

1. **Cache staleness**: If a device is connected/disconnected within 30 seconds, the cache won't reflect the change until TTL expires or user manually refreshes.
2. **Memory overhead**: Device list is cloned multiple times (global cache → DeviceSelector → StartupDialog), but device structs are small so impact is minimal.
3. **No explicit cache invalidation**: Cache only expires via TTL; no manual invalidation when device state changes detected (acceptable for MVP).
