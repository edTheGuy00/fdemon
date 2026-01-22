## Task: Pre-populate Connected Devices from Cache on Dialog Open

**Objective**: When the NewSessionDialog opens, immediately populate the device list from the existing cache instead of starting with an empty list and loading state.

**Depends on**: None

**Bug Reference**: Bug 1 - Connected Devices Not Cached on First Launch

### Scope

- `src/app/state.rs`: Modify `show_new_session_dialog()` to check and use device cache

### Details

Currently, `show_new_session_dialog()` creates a fresh `NewSessionDialogState` without checking if devices are already cached:

```rust
// Current implementation (src/app/state.rs:399-401)
pub fn show_new_session_dialog(&mut self, configs: LoadedConfigs) {
    self.new_session_dialog_state = NewSessionDialogState::new(configs);
    self.ui_mode = UiMode::NewSessionDialog;
}
```

The fix should:
1. After creating `NewSessionDialogState`, check `self.get_cached_devices()`
2. If cache exists and is valid, call `target_selector.set_connected_devices(cached_devices)`
3. Keep triggering background refresh (don't change that behavior)

**Implementation:**

```rust
pub fn show_new_session_dialog(&mut self, configs: LoadedConfigs) {
    self.new_session_dialog_state = NewSessionDialogState::new(configs);

    // Pre-populate from cache if available
    if let Some(cached_devices) = self.get_cached_devices() {
        self.new_session_dialog_state
            .target_selector
            .set_connected_devices(cached_devices.clone());
    }

    self.ui_mode = UiMode::NewSessionDialog;
}
```

**Key Files to Reference:**
- `src/app/state.rs:493-517` - `get_cached_devices()` and `set_device_cache()` methods
- `src/tui/widgets/new_session_dialog/target_selector.rs:204-219` - `set_connected_devices()` method

### Acceptance Criteria

1. Second dialog open shows devices instantly (no "Discovering devices..." spinner)
2. Cache is used if valid (within TTL)
3. Background refresh still occurs after dialog opens
4. First launch still shows loading state (cache empty)
5. No regression in device selection behavior

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_new_session_dialog_uses_cached_devices() {
        let mut state = AppState::default();
        let configs = LoadedConfigs::default();

        // Simulate cached devices
        let devices = vec![
            Device { id: "device1".to_string(), name: "Test Device".to_string(), .. },
        ];
        state.set_device_cache(devices.clone());

        // Open dialog
        state.show_new_session_dialog(configs);

        // Verify devices are pre-populated
        assert_eq!(
            state.new_session_dialog_state.target_selector.connected_devices.len(),
            1
        );
        assert!(!state.new_session_dialog_state.target_selector.loading);
    }

    #[test]
    fn test_show_new_session_dialog_empty_cache_shows_loading() {
        let mut state = AppState::default();
        let configs = LoadedConfigs::default();

        // No cached devices
        state.show_new_session_dialog(configs);

        // Verify loading state
        assert!(state.new_session_dialog_state.target_selector.connected_devices.is_empty());
        assert!(state.new_session_dialog_state.target_selector.loading);
    }
}
```

### Notes

- The `get_cached_devices()` method already handles TTL validation
- `set_connected_devices()` sets `loading = false`, which is correct behavior when cache is used
- Background refresh is triggered separately in `runner.rs` - no changes needed there
- Consider: Should we show a subtle "refreshing..." indicator when using cache but refresh is pending?

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (to be filled after implementation)

**Implementation Details:**

(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` -
- `cargo check` -
- `cargo clippy` -
- `cargo test` -

**Notable Decisions:**
- (to be filled after implementation)

**Risks/Limitations:**
- (to be filled after implementation)
