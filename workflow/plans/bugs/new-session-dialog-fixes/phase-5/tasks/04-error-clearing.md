## Task: Add Error Clearing to set_bootable_devices()

**Objective**: Clear the error state when bootable devices are successfully loaded, matching the behavior of `set_connected_devices()`.

**Priority**: Major

**Depends on**: None

### Scope

- `src/tui/widgets/new_session_dialog/target_selector.rs`: `set_bootable_devices()` method (around line 228)

### Problem Analysis

**`set_connected_devices()` - DOES clear error (lines 205-218):**
```rust
pub fn set_connected_devices(&mut self, devices: Vec<Device>) {
    self.connected_devices = devices;
    self.loading = false;
    self.error = None;  // ← ERROR CLEARED
    self.invalidate_cache();
    self.scroll_offset = 0;
    // ...
}
```

**`set_bootable_devices()` - MISSING error clear (lines 222-239):**
```rust
pub fn set_bootable_devices(
    &mut self,
    ios_simulators: Vec<IosSimulator>,
    android_avds: Vec<AndroidAvd>,
) {
    self.ios_simulators = ios_simulators;
    self.android_avds = android_avds;
    self.bootable_loading = false;
    // ← NO self.error = None; HERE
    self.invalidate_cache();
    self.scroll_offset = 0;
    // ...
}
```

### Why This Matters

If a previous bootable device discovery failed (setting an error message), and then a subsequent discovery succeeds, the error message persists. This causes users to see stale error messages even though the operation succeeded.

### Solution

Add `self.error = None;` to `set_bootable_devices()` to clear any previous error state.

### Implementation

**In `set_bootable_devices()` (target_selector.rs, around line 228):**

```rust
pub fn set_bootable_devices(
    &mut self,
    ios_simulators: Vec<IosSimulator>,
    android_avds: Vec<AndroidAvd>,
) {
    self.ios_simulators = ios_simulators;
    self.android_avds = android_avds;
    self.bootable_loading = false;
    self.error = None;  // ← ADD THIS LINE
    self.invalidate_cache();
    self.scroll_offset = 0;

    // Reset selection if on bootable tab
    if self.active_tab == TargetTab::Bootable {
        let max_index = self.compute_flat_list().len().saturating_sub(1);
        if self.selected_index > max_index {
            self.selected_index = self.first_selectable_index();
        }
    }
}
```

### Acceptance Criteria

1. `set_bootable_devices()` clears `self.error` when called
2. Error messages don't persist after successful bootable discovery
3. Behavior matches `set_connected_devices()`
4. All existing tests pass

### Testing

```bash
cargo test target_selector
cargo test bootable
```

Add test:
```rust
#[test]
fn test_set_bootable_devices_clears_error() {
    let mut state = TargetSelectorState::default();
    state.error = Some("Previous error".to_string());

    state.set_bootable_devices(vec![], vec![]);

    assert!(state.error.is_none());
}
```

### Notes

- Consider if there should be a separate `bootable_error` field (future enhancement)
- For now, a single `error` field for the entire target selector is acceptable

---

## Completion Summary

**Status:** Not Started
