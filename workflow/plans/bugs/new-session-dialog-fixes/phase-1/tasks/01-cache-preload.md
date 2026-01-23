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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/state.rs` | Modified `show_new_session_dialog()` to check and use device cache if available |
| `src/app/state.rs` | Added 5 unit tests for cache preload behavior |

### Notable Decisions/Tradeoffs

1. **Cache Check Before Dialog Creation**: The cache check happens immediately after creating `NewSessionDialogState`, ensuring the dialog state is initialized with fresh data if available while maintaining the existing creation pattern.

2. **Clone Cached Devices**: We clone the cached devices when populating the dialog to avoid borrowing issues and maintain data independence between the global cache and dialog state.

3. **Automatic Loading State Management**: When `set_connected_devices()` is called with cached data, it automatically sets `loading = false`, which gives us the desired instant display behavior without additional state management.

### Testing Performed

- `cargo fmt` - PASSED
- `cargo check` - PASSED
- `cargo clippy -- -D warnings` - PASSED
- `cargo test --lib test_show_new_session_dialog` - PASSED (5 tests)
- `cargo test --lib` - 1447 passed, 1 failed (pre-existing failure unrelated to changes)

**Test Coverage:**
- `test_show_new_session_dialog_uses_cached_devices` - Verifies devices are pre-populated from cache
- `test_show_new_session_dialog_empty_cache_shows_loading` - Verifies loading state when cache is empty
- `test_show_new_session_dialog_expired_cache_shows_loading` - Verifies expired cache is not used
- `test_show_new_session_dialog_fresh_cache_within_ttl` - Verifies fresh cache is used
- `test_show_new_session_dialog_multiple_opens_use_cache` - Verifies cache persists across multiple opens

### Risks/Limitations

1. **Cache TTL Trade-off**: The 30-second TTL balances responsiveness with freshness. Devices might change between opens, but background refresh ensures eventual consistency.

2. **Pre-existing Test Failure**: One unrelated test (`test_truncate_middle_very_short`) was already failing before implementation. This is a separate issue in the TUI widget layer, not related to cache preload functionality.
