## Task: Remove Duplicate Cache Checking Logic

**Objective**: Eliminate redundant cache checking that occurs in both `show_new_session_dialog()` and `handle_open_new_session_dialog()`.

**Priority**: Critical

**Depends on**: None

### Scope

- `src/app/state.rs`: Remove cache checking from `show_new_session_dialog()` (lines 419-432)

### Problem Analysis

Cache is checked and devices populated in **two** locations:

1. **`show_new_session_dialog()`** (state.rs:419-432):
```rust
// Pre-populate from cache if available (Bug Fix: Task 01)
if let Some(cached_devices) = self.get_cached_devices() {
    self.new_session_dialog_state
        .target_selector
        .set_connected_devices(cached_devices.clone());
}

// Pre-populate bootable devices from cache if available (Bug Fix: Task 03)
if let Some((simulators, avds)) = self.get_cached_bootable_devices() {
    self.new_session_dialog_state
        .target_selector
        .set_bootable_devices(simulators, avds);
}
```

2. **`handle_open_new_session_dialog()`** (navigation.rs:177-188):
```rust
// Check cache first (Task 04 - Device Cache Usage)
if let Some(cached_devices) = state.get_cached_devices() {
    // ... logging ...
    state
        .new_session_dialog_state
        .target_selector
        .set_connected_devices(cached_devices.clone());

    return UpdateResult::action(UpdateAction::RefreshDevicesBackground);
}
```

### Solution

Remove the cache checking block from `show_new_session_dialog()`. Keep the cache checking in `handle_open_new_session_dialog()` because:

1. It also handles the background refresh trigger
2. It provides debug logging about cache age
3. It's the proper TEA handler location for this logic

### Implementation

**Delete these lines from `show_new_session_dialog()` (state.rs:419-432):**

```rust
// DELETE: Pre-populate from cache if available (Bug Fix: Task 01)
if let Some(cached_devices) = self.get_cached_devices() {
    self.new_session_dialog_state
        .target_selector
        .set_connected_devices(cached_devices.clone());
}

// DELETE: Pre-populate bootable devices from cache if available (Bug Fix: Task 03)
if let Some((simulators, avds)) = self.get_cached_bootable_devices() {
    self.new_session_dialog_state
        .target_selector
        .set_bootable_devices(simulators, avds);
}
```

### Acceptance Criteria

1. Only one location checks and populates device cache (navigation.rs handler)
2. Only one location checks and populates bootable cache (navigation.rs handler)
3. Dialog still shows cached devices immediately when opened
4. Background refresh still triggers after cache population
5. All existing tests pass

### Testing

```bash
cargo test new_session
cargo test cache
```

Verify manually:
1. Open new session dialog - cached devices should appear immediately
2. Subsequent opens should not show duplicate loading states

### Notes

- The navigation handler should also be updated to handle bootable cache if not already
- Consider adding a comment explaining why cache check is only in handler

---

## Completion Summary

**Status:** Not Started
