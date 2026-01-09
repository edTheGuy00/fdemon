## Task: Create Unit Tests for Boolean Toggle Handler

**Objective**: Add unit tests in the handler module that verify `SettingsToggleBool` message handling and demonstrate the bug at the unit level.

**Depends on**: None (can run in parallel with Task 01)

### Scope

- `src/app/handler/update.rs`: Add tests in the `#[cfg(test)] mod tests` section
- Focus on the `SettingsToggleBool` handler at lines 1102-1107

### Details

The bug is in `src/app/handler/update.rs` around lines 1102-1107. The `SettingsToggleBool` handler:
- **Current behavior**: Sets `state.settings.dirty = true` but does NOT flip the boolean value
- **Expected behavior**: Should flip the boolean AND set dirty flag

Create unit tests that:
1. Test the `SettingsToggleBool` message handler directly
2. Verify the boolean value changes after handling the message
3. Verify the dirty flag is set

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "BUG: Boolean toggle not implemented - see workflow/plans/bugs/boolean-toggle/BUG.md"]
    fn test_settings_toggle_bool_flips_value() {
        // Setup: Create AppState with a known boolean setting value
        let mut state = AppState::default();
        // Set auto_reload to true initially
        state.settings.config.watcher.auto_reload = true;

        // Create the toggle message for auto_reload
        let message = Message::SettingsToggleBool {
            key: "auto_reload".to_string()
        };

        // Handle the message
        let (new_state, _action) = update(state, message);

        // EXPECTED: Value should be flipped to false
        // ACTUAL: Value remains true (bug)
        assert_eq!(new_state.settings.config.watcher.auto_reload, false);

        // Dirty flag should be set (this part works)
        assert!(new_state.settings.dirty);
    }

    #[test]
    fn test_settings_toggle_bool_sets_dirty_flag() {
        // This test should PASS - dirty flag IS set correctly
        let mut state = AppState::default();
        state.settings.dirty = false;

        let message = Message::SettingsToggleBool {
            key: "auto_reload".to_string()
        };

        let (new_state, _action) = update(state, message);

        // Dirty flag is set (this works)
        assert!(new_state.settings.dirty);
    }
}
```

### Acceptance Criteria

1. Unit test for value flipping exists and is marked `#[ignore]`
2. Unit test for dirty flag exists and passes (demonstrates partial functionality)
3. Test comments clearly document expected vs actual behavior
4. Tests use proper setup with realistic AppState

### Testing

```bash
# Run passing tests
cargo test test_settings_toggle_bool_sets_dirty_flag

# Run ignored test to see it fail
cargo test test_settings_toggle_bool_flips_value -- --ignored
```

### Notes

- Review existing handler tests for patterns to follow
- The exact structure of `SettingsToggleBool` message may need verification
- Consider testing multiple boolean settings if the handler is generic
- Document the bug's root cause in code comments

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/tests.rs` | Added two unit tests for `SettingsToggleBool` message handler (lines 2102-2162) |

### Implementation Details

Added two unit tests in the handler test suite:

1. **`test_settings_toggle_bool_flips_value`** (lines 2106-2141)
   - Marked with `#[ignore]` attribute with bug reference
   - Tests that boolean value should be flipped (FAILS - demonstrates bug)
   - Uses `state.settings.watcher.auto_reload` as test subject
   - Documents expected vs actual behavior in comments
   - Tests toggling from true→false and back to true→false

2. **`test_settings_toggle_bool_sets_dirty_flag`** (lines 2143-2162)
   - Tests that dirty flag is correctly set (PASSES)
   - Verifies partial functionality works correctly
   - Clean test setup with explicit initial state

Both tests follow existing patterns in the test file and include comprehensive documentation of the bug.

### Testing Performed

✅ **Passing test:**
```bash
cargo test test_settings_toggle_bool_sets_dirty_flag --lib
# Result: ok. 1 passed; 0 failed
```

✅ **Ignored test demonstrates bug:**
```bash
cargo test test_settings_toggle_bool_flips_value --lib -- --ignored
# Result: FAILED (as expected)
# Error: assertion failed: auto_reload should be flipped to false, but remains true
#   left: true, right: false
```

✅ **All unit tests still pass:**
```bash
cargo test --lib
# Result: ok. 1329 passed; 0 failed; 4 ignored
```

✅ **Full verification:**
```bash
cargo fmt && cargo check && cargo clippy -- -D warnings
# All passed with no warnings
```

### Notable Decisions/Tradeoffs

1. **Test Subject Selection**: Used `state.settings.watcher.auto_reload` as the test boolean because:
   - It's a simple boolean setting with clear semantics
   - Accessible directly from `AppState`
   - Represents a common use case for boolean toggles

2. **Test Structure**: Followed existing test patterns in `tests.rs`:
   - Used `AppState::new()` for state initialization
   - Called `update()` directly with message
   - Clear assertion messages documenting expected vs actual behavior

3. **Documentation**: Added extensive comments in the ignored test explaining:
   - What the bug is (no boolean value toggle implementation)
   - Where the bug is (update.rs:1102-1107)
   - Expected vs actual behavior
   - Why the test is ignored

### Risks/Limitations

None. Tests are isolated unit tests that don't affect production code or other tests.
