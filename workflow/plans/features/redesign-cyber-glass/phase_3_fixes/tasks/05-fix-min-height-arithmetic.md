## Task: Fix min_height() Arithmetic

**Objective**: Correct the `min_height()` return value to match the actual layout. The current value of 21 is wrong — the comment arithmetic sums to 23, and the actual layout (including the spacer before the button) requires 24 rows. After Task 01 adds DartDefines, this will increase further.

**Depends on**: Task 01 (adding DartDefines changes the layout height)

**Review Reference**: REVIEW.md #6 (Major), ACTION_ITEMS.md #6

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` line 769-771: Update `min_height()` return value and comment

### Details

**Current code** (line 769-771):
```rust
pub fn min_height() -> u16 {
    21 // Spacer + config(4) + spacer + mode(4) + spacer + flavor(4) + spacer + entry(4) + button(3)
}
```

**Arithmetic errors**:
1. Comment lists: 1 + 4 + 1 + 4 + 1 + 4 + 1 + 4 + 3 = **23**, not 21
2. Comment omits the spacer between Entry Point and Launch button (added at line 788: `chunks[7].y + chunks[7].height + 1`)
3. Correct sum without DartDefines: 1 + 4 + 1 + 4 + 1 + 4 + 1 + 4 + 1 + 3 = **24**

**After Task 01** (adding DartDefines field):
- New layout adds: spacer(1) + DartDefines(4) = 5 additional rows
- New total: 1 + 4 + 1 + 4 + 1 + 4 + 1 + 4 + 1 + 4 + 1 + 3 = **29**

**Fix**: Update the return value and comment to match the actual layout after Task 01 is complete. The exact value depends on the final layout from Task 01.

```rust
pub fn min_height() -> u16 {
    29 // spacer(1) + config(4) + spacer(1) + mode(4) + spacer(1) + flavor(4) + spacer(1) + entry(4) + spacer(1) + dart_defines(4) + spacer(1) + button(3)
}
```

### Acceptance Criteria

1. `min_height()` returns the correct value matching actual layout constraints
2. Comment accurately documents the arithmetic with all components listed
3. No button clipping in tight terminal heights at the minimum size
4. `cargo check -p fdemon-tui` passes

### Testing

- Add a unit test asserting `min_height()` equals the sum of all layout components
- Verify the launch button is fully visible at exactly `min_height()` rows

### Notes

- This task MUST be done after Task 01, since adding DartDefines changes the total height.
- The `min_height()` value is used by the parent dialog to determine if compact mode should be used. An incorrect value can cause the button to be clipped in horizontal mode or prevent compact mode from triggering when it should.
- Verify where `min_height()` is called to understand the impact of changing this value.
