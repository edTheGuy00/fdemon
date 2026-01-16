## Task: Fix Render Tests

**Objective**: Update render tests that reference deleted `UiMode::DeviceSelector` and `device_selector` field.

**Depends on**: 08-remove-deprecated-handlers

**Estimated Time**: 30 minutes

**Priority**: 🟠 Major

**Source**: Architecture Enforcer

### Scope

- `src/tui/render/tests.rs`: Fix 7+ tests referencing deleted types

### Issues to Fix

**Snapshot tests to update/replace:**

1. **Line 80**: `snapshot_device_selector_empty()` → Remove or convert to `NewSessionDialog`
2. **Line 92**: `snapshot_device_selector_with_devices()` → Remove or convert
3. **Line 207**: `snapshot_compact_device_selector()` → Remove or convert

**Transition tests to update:**

4. **Line 323**: `test_transition_normal_to_device_selector()` → Update to `NewSessionDialog`
5. **Line 369**: `test_transition_device_selector_to_normal()` → Update from `NewSessionDialog`
6. **Line 468**: Test using `UiMode::DeviceSelector` → Update
7. **Line 503**: Mode list containing `UiMode::DeviceSelector` → Update

### Process

1. For each DeviceSelector snapshot test:
   - Option A: Remove if redundant with existing NewSessionDialog tests
   - Option B: Convert to NewSessionDialog equivalent

2. For transition tests:
   - Replace `UiMode::DeviceSelector` with `UiMode::NewSessionDialog`
   - Update state setup to use `new_session_dialog_state`
   - Update assertions

3. Run tests and update insta snapshots if needed:
   ```bash
   cargo test render
   cargo insta review
   ```

### Example Updates

**Before:**
```rust
fn snapshot_device_selector_empty() {
    let mut state = AppState::default();
    state.ui_mode = UiMode::DeviceSelector;
    // ...
}
```

**After (Option A - Remove):**
Delete test if NewSessionDialog tests already cover this.

**After (Option B - Convert):**
```rust
fn snapshot_new_session_dialog_empty() {
    let mut state = AppState::default();
    state.ui_mode = UiMode::NewSessionDialog;
    // ...
}
```

### Acceptance Criteria

1. `cargo test render` compiles without errors
2. All render tests pass
3. No references to `UiMode::DeviceSelector`
4. No references to `device_selector` state field
5. Snapshot tests reflect NewSessionDialog UI (if converted)

### Testing

```bash
cargo test render
cargo insta review  # Accept/reject snapshot changes
```

### Notes

- Review existing NewSessionDialog tests before converting - avoid duplicate coverage
- Some DeviceSelector-specific tests may not have NewSessionDialog equivalents (device list rendering) - create new tests if coverage is needed
- Transition tests are important for verifying mode switching works correctly

---

## Completion Summary

**Status:** Not Started
