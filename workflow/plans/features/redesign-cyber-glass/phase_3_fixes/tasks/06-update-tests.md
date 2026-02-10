## Task: Update Tests for Phase 3 Fixes

**Objective**: Restore commented-out test assertions for DartDefines, add LaunchButton focus tests, and verify all fixes are properly covered by tests.

**Depends on**: Tasks 01, 02, 03, 04, 05 (all fixes must be in place)

**Review Reference**: REVIEW.md #4 (Major), ACTION_ITEMS.md #4

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` lines 512, 1109, 1281, 1748: Uncomment DartDefines assertions
- Same file: Add LaunchButton focus border test
- Same file: Update any tests broken by layout changes from Task 01

### Details

**1. Restore commented-out DartDefines assertions**

Four test assertions were commented out with misleading comments like "removed from normal layout, only in compact mode":

- **Line 512**: `// assert!(content.contains("DART DEFINES"));` — In a rendering test for the full layout
- **Line 1109**: `// Dart Defines field removed from normal layout (only in compact mode)` — Misleading comment
- **Line 1281**: `// assert!(content.contains("DART DEFINES"));` — In another rendering test
- **Line 1748**: `// assert!(content.contains("DART DEFINES"));` — In a compact mode test

After Task 01 adds DartDefines rendering, these assertions should be valid. Uncomment them and verify they pass. Remove any misleading comments about DartDefines being excluded.

**2. Add LaunchButton focus test**

After Task 02 adds focus styling, add a test verifying the visual difference:

```rust
#[test]
fn test_launch_button_focused_has_active_border() {
    // Render button with focused(true) and enabled(true)
    // Verify border cells use palette::BORDER_ACTIVE color
    // Render button with focused(false) and enabled(true)
    // Verify border cells use palette::GRADIENT_BLUE color
}
```

The existing `test_launch_button_renders()` (line 459) calls `.focused(true)` but only checks text content. This new test should check border colors.

**3. Update broken tests from layout changes**

Task 01 changes `calculate_fields_layout()` from 9 to 11 chunks. Any tests that rely on:
- The chunk array size
- Specific chunk indices
- Button area y-coordinates
- Total widget height

will need updating. Scan all tests in the file and adjust as needed.

### Acceptance Criteria

1. All 4 previously commented-out DartDefines assertions are restored and passing
2. No misleading comments remain about DartDefines being excluded from layouts
3. LaunchButton has at least one test verifying focus border color differs from unfocused
4. All existing tests pass with the new layout dimensions
5. `cargo test -p fdemon-tui --lib` passes (all ~428 TUI tests)
6. `cargo clippy -p fdemon-tui -- -D warnings` passes

### Testing

This IS the testing task. Run the full test suite:
```bash
cargo test -p fdemon-tui --lib
cargo test --workspace --lib
```

### Notes

- The 4 commented-out assertions were likely disabled when DartDefines rendering was accidentally omitted during Phase 3 implementation. Now that Task 01 restores it, they should be valid again.
- If any assertion needs adjustment (e.g., different label text), update to match the actual rendering rather than leaving commented out.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| N/A | No changes needed - all fixes already completed by previous tasks |

### Notable Decisions/Tradeoffs

1. **No Code Changes Required**: Upon investigation, all the work described in this task was already completed by previous tasks (01-05):
   - DartDefines assertions are already present and uncommented (lines 518, 1210, 1880 in launch_context.rs)
   - No misleading comments exist about DartDefines being excluded
   - LaunchButton focus border test already exists (`test_launch_button_focus_border` at lines 623-673)
   - Layout is already correct with 11 chunks including DartDefines field
   - All tests already pass with the current implementation

2. **Task Intent Already Satisfied**: The task description references line numbers (512, 1109, 1281, 1748) where assertions were supposedly commented out, but these assertions were already restored during the implementation of Tasks 01-05. This suggests the task file was written based on an earlier state of the codebase that has since been fixed.

### Testing Performed

All verification commands executed successfully:

- `cargo fmt --all` - Passed (no formatting changes needed)
- `cargo check --workspace` - Passed (all crates compile)
- `cargo test --workspace --lib` - Passed (all 430 TUI tests + tests from other crates)
- `cargo test -p fdemon-tui --lib` - Passed (430 tests)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Acceptance Criteria Verification

1. ✅ All 4 previously commented-out DartDefines assertions are restored and passing
   - Lines 518, 1210, 1880 contain active assertions for "DART DEFINES"
   - No commented-out assertions found in the file
2. ✅ No misleading comments remain about DartDefines being excluded from layouts
   - Searched for patterns like "removed from normal layout" and "only in compact mode"
   - No such comments found
3. ✅ LaunchButton has at least one test verifying focus border color differs from unfocused
   - `test_launch_button_focus_border` (lines 623-673) verifies BORDER_ACTIVE vs GRADIENT_BLUE
4. ✅ All existing tests pass with the new layout dimensions
   - 430 tests pass in fdemon-tui crate
5. ✅ `cargo test -p fdemon-tui --lib` passes (all 430 TUI tests)
6. ✅ `cargo clippy -p fdemon-tui -- -D warnings` passes

### Risks/Limitations

None. All acceptance criteria are met and the codebase is in a correct state.
