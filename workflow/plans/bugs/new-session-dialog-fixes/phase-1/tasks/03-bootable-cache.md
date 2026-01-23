## Task: Add Bootable Device Caching

**Objective**: Cache bootable devices (iOS simulators, Android AVDs) similar to connected devices, so they appear instantly when the dialog reopens.

**Depends on**: Task 02 (Bootable Discovery at Startup)

**Bug Reference**: Bug 2 - Bootable Devices List Never Populates on First Open

### Scope

- `src/app/state.rs`: Add bootable device cache fields and methods
- `src/app/handler/update.rs`: Update `BootableDevicesDiscovered` handler to cache results
- `src/app/state.rs`: Modify `show_new_session_dialog()` to pre-populate bootable devices from cache

### Details

Currently, connected devices are cached in `AppState`:
```rust
// src/app/state.rs
pub device_cache: Option<Vec<Device>>,
pub devices_last_updated: Option<Instant>,
```

Bootable devices (iOS simulators and Android AVDs) are NOT cached. Every time the dialog opens, they start empty and require a fresh discovery.

**Implementation:**

**Step 1:** Add bootable cache fields to `AppState` (`src/app/state.rs`):

```rust
pub struct AppState {
    // ... existing fields ...

    // Connected device cache
    pub device_cache: Option<Vec<Device>>,
    pub devices_last_updated: Option<Instant>,

    // Bootable device cache (NEW)
    pub ios_simulators_cache: Option<Vec<IosSimulator>>,
    pub android_avds_cache: Option<Vec<AndroidAvd>>,
    pub bootable_last_updated: Option<Instant>,
}
```

**Step 2:** Add cache methods (`src/app/state.rs`):

```rust
/// Get cached bootable devices if still valid (within TTL)
pub fn get_cached_bootable_devices(&self) -> Option<(Vec<IosSimulator>, Vec<AndroidAvd>)> {
    if let (Some(simulators), Some(avds), Some(last_updated)) = (
        &self.ios_simulators_cache,
        &self.android_avds_cache,
        self.bootable_last_updated,
    ) {
        if last_updated.elapsed() < DEVICE_CACHE_TTL {
            return Some((simulators.clone(), avds.clone()));
        }
    }
    None
}

/// Update the bootable device cache
pub fn set_bootable_cache(&mut self, simulators: Vec<IosSimulator>, avds: Vec<AndroidAvd>) {
    self.ios_simulators_cache = Some(simulators);
    self.android_avds_cache = Some(avds);
    self.bootable_last_updated = Some(Instant::now());
}
```

**Step 3:** Update `BootableDevicesDiscovered` handler (`src/app/handler/update.rs`):

```rust
Message::BootableDevicesDiscovered { ios_simulators, android_avds } => {
    // Cache bootable devices (NEW)
    state.set_bootable_cache(ios_simulators.clone(), android_avds.clone());

    // Update dialog state (existing)
    if state.ui_mode == UiMode::Startup || state.ui_mode == UiMode::NewSessionDialog {
        state.new_session_dialog_state
            .target_selector
            .set_bootable_devices(ios_simulators, android_avds);
    }

    UpdateResult::none()
}
```

**Step 4:** Update `show_new_session_dialog()` (`src/app/state.rs`):

```rust
pub fn show_new_session_dialog(&mut self, configs: LoadedConfigs) {
    self.new_session_dialog_state = NewSessionDialogState::new(configs);

    // Pre-populate connected devices from cache (Task 01)
    if let Some(cached_devices) = self.get_cached_devices() {
        self.new_session_dialog_state
            .target_selector
            .set_connected_devices(cached_devices.clone());
    }

    // Pre-populate bootable devices from cache (NEW - Task 03)
    if let Some((simulators, avds)) = self.get_cached_bootable_devices() {
        self.new_session_dialog_state
            .target_selector
            .set_bootable_devices(simulators, avds);
    }

    self.ui_mode = UiMode::NewSessionDialog;
}
```

**Key Files to Reference:**
- `src/app/state.rs:335-346` - Existing `device_cache` fields
- `src/app/state.rs:493-517` - Existing cache methods
- `src/app/handler/update.rs:1049-1062` - `BootableDevicesDiscovered` handler
- `src/tui/widgets/new_session_dialog/target_selector.rs:221-240` - `set_bootable_devices()`
- `src/daemon/simulator.rs` - `IosSimulator` type
- `src/daemon/avd.rs` - `AndroidAvd` type

### Acceptance Criteria

1. Second dialog open shows bootable devices instantly (from cache)
2. Cache is used if valid (within TTL of 5 seconds)
3. Background refresh still occurs after dialog opens
4. First launch still shows loading state until discovery completes
5. "r" key refreshes bootable devices and updates cache
6. No regression in connected device caching

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_bootable_cache() {
        let mut state = AppState::default();
        let simulators = vec![IosSimulator {
            udid: "test-udid".to_string(),
            name: "iPhone 15".to_string(),
            // ...
        }];
        let avds = vec![];

        state.set_bootable_cache(simulators.clone(), avds.clone());

        assert!(state.ios_simulators_cache.is_some());
        assert!(state.bootable_last_updated.is_some());
    }

    #[test]
    fn test_get_cached_bootable_devices_valid() {
        let mut state = AppState::default();
        let simulators = vec![IosSimulator { /* ... */ }];
        let avds = vec![];
        state.set_bootable_cache(simulators.clone(), avds.clone());

        let cached = state.get_cached_bootable_devices();
        assert!(cached.is_some());
        let (s, a) = cached.unwrap();
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn test_show_new_session_dialog_uses_bootable_cache() {
        let mut state = AppState::default();
        let configs = LoadedConfigs::default();

        // Pre-populate bootable cache
        let simulators = vec![IosSimulator { /* ... */ }];
        state.set_bootable_cache(simulators.clone(), vec![]);

        // Open dialog
        state.show_new_session_dialog(configs);

        // Verify bootable devices are pre-populated
        assert_eq!(
            state.new_session_dialog_state.target_selector.ios_simulators.len(),
            1
        );
        assert!(!state.new_session_dialog_state.target_selector.bootable_loading);
    }
}
```

### Notes

- Use the same `DEVICE_CACHE_TTL` constant (5 seconds) for consistency
- `set_bootable_devices()` already sets `bootable_loading = false`, which is correct
- Consider: Should bootable cache have a longer TTL since emulators change less frequently?
- Make sure to initialize new cache fields in `AppState::default()` and `AppState::new()`

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/state.rs` | Added bootable device cache fields (`ios_simulators_cache`, `android_avds_cache`, `bootable_last_updated`), added cache methods (`get_cached_bootable_devices()`, `set_bootable_cache()`), updated `show_new_session_dialog()` to pre-populate from cache, added 5 unit tests |
| `src/app/handler/update.rs` | Updated `BootableDevicesDiscovered` handler to cache results via `state.set_bootable_cache()` |
| `src/app/handler/tests.rs` | Added 2 integration tests: `test_bootable_devices_discovered_updates_cache()` and `test_bootable_cache_persists_across_dialog_reopens()` |

### Implementation Details

**Cache Fields Added:**
- `ios_simulators_cache: Option<Vec<IosSimulator>>` - Cached iOS simulators
- `android_avds_cache: Option<Vec<AndroidAvd>>` - Cached Android AVDs
- `bootable_last_updated: Option<Instant>` - Cache timestamp for TTL validation

**Cache Methods:**
- `get_cached_bootable_devices()` - Returns cached devices if within 30s TTL (same as connected devices)
- `set_bootable_cache()` - Updates cache with fresh discovery results

**Handler Changes:**
- `BootableDevicesDiscovered` now caches results before updating dialog state
- Cache persists across dialog opens/closes for instant display

**Dialog Behavior:**
- First open: Shows loading state until discovery completes
- Subsequent opens (within TTL): Shows cached devices instantly, background refresh still occurs
- "r" key refresh: Updates cache with fresh results

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo clippy -- -D warnings` - Passed (0 warnings)
- `cargo test --lib state::tests` - Passed (32/32 tests, including 5 new bootable cache tests)
- `cargo test --lib bootable` - Passed (all 20 bootable-related tests)
- New handler integration tests - Passed (2/2 tests)

**Test Coverage:**
- Cache set/get operations
- Cache TTL validation
- Dialog pre-population from cache
- Cache persistence across dialog reopens
- Handler updates cache on discovery
- Connected device cache still works (no regression)

### Notable Decisions/Tradeoffs

1. **Cache TTL Decision**: Used 30 seconds (same as connected devices) instead of 5 seconds mentioned in task notes. This provides better consistency and is appropriate since bootable device lists change infrequently (requires manual simulator/AVD creation/deletion).

2. **Cache Structure**: Stored iOS simulators and Android AVDs separately (not as unified `BootableDevice` list) to match the discovery API structure and avoid unnecessary type conversions.

3. **Clone vs Reference**: `get_cached_bootable_devices()` returns `Option<(Vec<IosSimulator>, Vec<AndroidAvd>)>` (cloned) rather than references, matching the pattern of `set_bootable_devices()` API which takes owned values.

### Risks/Limitations

1. **Cache Staleness**: Cache shows stale data for up to 30 seconds. This is acceptable because:
   - Background refresh still occurs after dialog opens
   - Bootable device changes are rare (manual simulator/AVD management)
   - User can always press "r" to force refresh

2. **Memory Usage**: Cache holds full bootable device lists in memory. Impact is minimal because:
   - Typical simulator/AVD counts are small (5-10 per type)
   - Cache is cleared after 30 seconds of inactivity
   - Struct sizes are small (strings + primitives)
