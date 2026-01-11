## Task: Update Snapshot Tests

**Objective**: Update any snapshot tests that may be affected by the startup flow changes, particularly tests that capture initial UI state.

**Depends on**: 01-remove-dead-code

**Estimated Time**: 0.5 hours

### Scope

- `src/tui/render/tests.rs`: Review and update snapshot tests

### Details

#### Potentially Affected Tests

Review these test categories in `render/tests.rs`:

1. **Initial state tests**
   - Tests that capture the UI when app first starts
   - May need to show "Not Connected" instead of loading

2. **Loading screen tests**
   - Tests that verify loading screen rendering
   - Should still pass (loading is still used, just triggered differently)

3. **State transition tests**
   - Tests that verify transitions between UI modes
   - May need updates for Normal → Loading → Running flow

#### Investigation Steps

1. **Run existing snapshot tests:**
   ```bash
   cargo test render::tests -- --nocapture
   ```

2. **Identify failures:**
   - Look for mismatches in expected vs actual snapshots
   - Check if failures are due to startup flow changes

3. **Update snapshots:**
   ```bash
   # If using insta for snapshots:
   cargo insta review

   # Or regenerate manually:
   cargo test render::tests -- --nocapture
   # Then update expected strings in test file
   ```

#### Common Updates Needed

**Test: Initial Normal Mode State**
```rust
#[test]
fn test_initial_normal_mode() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Normal;
    // No sessions

    let frame = render_test_frame(&state);

    // Should show "Not Connected" and "Press + to start"
    assert!(frame.contains("Not Connected"));
    assert!(frame.contains("Press + to start"));
}
```

**Test: Loading Screen**
```rust
#[test]
fn test_loading_screen() {
    let mut state = AppState::new();
    state.set_loading_phase("Detecting devices...");

    let frame = render_test_frame(&state);

    // Should show loading spinner and message
    assert!(frame.contains("Detecting devices"));
    // Spinner character varies, just check loading mode
    assert_eq!(state.ui_mode, UiMode::Loading);
}
```

#### If No Changes Needed

If all snapshot tests pass without modification, document that fact and mark task complete.

### Acceptance Criteria

1. All snapshot tests reviewed
2. Failed tests identified and updated
3. New snapshots reflect correct behavior
4. `cargo test render::tests` passes
5. No visual regressions in test output

### Testing

```bash
# Run all render tests
cargo test render::tests

# Run with output for debugging
cargo test render::tests -- --nocapture

# Run specific test
cargo test test_initial_normal_mode
```

### Notes

- Snapshot tests may use `insta` crate or custom assertion
- Check `src/tui/render/tests.rs` for test structure
- If tests use golden files, those may need updating too
- Document any behavioral changes observed in test output

### Phase 3 Impact

**Already completed during Phase 3:**
- Loading snapshot (`flutter_demon__tui__render__tests__loading.snap`) was updated
- Loading screen now shows as an overlay on top of normal UI (header, log area, status bar visible)
- The snapshot shows the centered loading modal with spinner

**Remaining work:**
- Verify all other snapshot tests still pass
- No additional changes expected since only loading behavior changed

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (pending)

**Tests Updated:**
- (pending)

**Tests Passing:**
- (pending)

**Notable Changes:**

(pending)
