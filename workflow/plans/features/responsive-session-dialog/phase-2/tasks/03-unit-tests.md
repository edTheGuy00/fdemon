## Task: Add Unit Tests for Button Overflow Prevention

**Objective**: Add unit tests that verify the launch button never renders outside the dialog bounds at various terminal heights, including edge cases where `area.height < min_height()`. Update any existing arithmetic tests that are affected by the layout slot count change.

**Depends on**: 02-render-full-use-layout-button

**Estimated Time**: 2 hours

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs`: Add tests in `launch_context_tests` module (starts at line ~1176), update `test_min_height_arithmetic` (line ~1330)

### Details

**Test 1: Button stays within bounds at `min_height()` (29 rows)**

Render `LaunchContextWithDevice` in full (non-compact) mode at exactly `Rect::new(0, 0, 50, 29)`. Assert that every cell written to the buffer is within the area bounds. Specifically, verify:
- The button text ("LAUNCH INSTANCE" or "SELECT DEVICE") appears in the buffer
- No cell is written at `y >= 29`

```rust
#[test]
fn test_render_full_button_within_bounds_at_min_height() {
    let buf_height = 29;
    let area = Rect::new(0, 0, 50, buf_height);
    let mut buf = Buffer::empty(area);

    let state = LaunchContextState::default();
    let icons = IconSet::unicode();
    let widget = LaunchContextWithDevice::new(&state, false, false, &icons)
        .compact(false);
    widget.render(area, &mut buf);

    // Verify button text is present
    let content = buffer_to_string(&buf);
    assert!(content.contains("SELECT DEVICE") || content.contains("LAUNCH INSTANCE"));

    // No writes outside area bounds (Buffer panics on out-of-bounds, so reaching here = pass)
}
```

**Test 2: Button doesn't overflow at various small heights**

Render at heights below `min_height()` (e.g., 20, 25) and verify no panic occurs and the buffer contains no data beyond the area bounds. Since Phase 1 normally routes to `render_compact()` at these heights, this test calls `render_full()` directly (via `.compact(false)`) to verify the layout fix works independently of the compact guard.

```rust
#[test]
fn test_render_full_no_overflow_at_small_heights() {
    for height in [15, 20, 25, 28] {
        let area = Rect::new(0, 0, 50, height);
        let mut buf = Buffer::empty(area);

        let state = LaunchContextState::default();
        let icons = IconSet::unicode();
        let widget = LaunchContextWithDevice::new(&state, false, false, &icons)
            .compact(false);
        widget.render(area, &mut buf);
        // If we reach here without panic, the button didn't overflow the buffer
    }
}
```

**Test 3: Button renders correctly at large heights**

At height 40, verify the button is positioned correctly and has the expected 3-row height. The button should be at `y = 27` (26 rows of fields + 1 spacer) with `height = 3`.

```rust
#[test]
fn test_render_full_button_position_at_large_height() {
    let area = Rect::new(0, 0, 50, 40);
    let mut buf = Buffer::empty(area);

    let state = LaunchContextState::default();
    let icons = IconSet::unicode();
    let widget = LaunchContextWithDevice::new(&state, false, true, &icons)
        .compact(false);
    widget.render(area, &mut buf);

    // Button text should appear at expected position
    let content = buffer_to_string(&buf);
    assert!(content.contains("LAUNCH INSTANCE"));
}
```

**Test 4: `calculate_fields_layout` returns correct slot count and positions**

Verify that `calculate_fields_layout()` returns 13 slots with the button at `[11]` having `height == 3` when given sufficient area.

```rust
#[test]
fn test_calculate_fields_layout_includes_button_slot() {
    let area = Rect::new(0, 0, 50, 40);
    let chunks = calculate_fields_layout(area);

    assert_eq!(chunks.len(), 13);
    assert_eq!(chunks[10].height, 1, "button spacer should be 1 row");
    assert_eq!(chunks[11].height, 3, "button slot should be 3 rows");
    assert_eq!(chunks[11].y, 26, "button should start at row 26 (after 25 field rows + 1 spacer)");
}
```

**Test 5: Update `test_min_height_arithmetic`**

The existing test (line ~1330) verifies `min_height() == 29` with a breakdown of component sizes. Update the arithmetic comment to reflect the new layout slot structure (13 slots instead of 11) if needed. The total should still be 29.

**Test 6: `LaunchContext` (without device) button bounds**

The simpler `LaunchContext` widget has the same fix. Add a parallel test:

```rust
#[test]
fn test_launch_context_button_within_bounds() {
    let area = Rect::new(0, 0, 50, 29);
    let mut buf = Buffer::empty(area);

    let state = LaunchContextState::default();
    let icons = IconSet::unicode();
    let widget = LaunchContext::new(&state, false, &icons);
    widget.render(area, &mut buf);
    // No panic = button stayed within bounds
}
```

### Acceptance Criteria

1. Tests cover button rendering at heights: 15, 20, 25, 28, 29 (min), 30, 40
2. No test produces a buffer overflow panic
3. At `min_height()` (29), the button text is visible in the rendered buffer
4. At large heights (40), the button is at the expected position
5. `calculate_fields_layout()` returns 13 slots with correct dimensions
6. Both `LaunchContext` and `LaunchContextWithDevice` are covered
7. Existing `test_min_height` and `test_min_height_arithmetic` still pass
8. All existing `launch_context_tests` pass without modification
9. `cargo test -p fdemon-tui` passes — all tests green
10. `cargo clippy --workspace -- -D warnings` passes

### Testing

Run the full test suite:
- `cargo test -p fdemon-tui` — all existing + new tests
- `cargo clippy --workspace -- -D warnings` — no new warnings

### Notes

- The test helper `buffer_to_string()` may already exist in the test module. Check for existing helper functions before creating new ones.
- Ratatui's `Buffer::empty(area)` creates a buffer exactly the size of `area`. Writing beyond `area` bounds will panic, which is what makes these tests effective — a panic means the button overflowed.
- The tests that render at heights below 29 with `compact(false)` are testing a scenario that won't normally happen in production (Phase 1's compact guard prevents it). However, they verify the defense-in-depth fix works correctly regardless of the caller's compact decision.
- Some existing tests may reference `chunks[10]` as the "rest" slot. After task 01, the rest slot moves to `chunks[12]`. Scan for and update any such references.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | Added 5 new tests in `launch_context_tests` module for button overflow prevention |

### Notable Decisions/Tradeoffs

1. **Used `Buffer::empty(area)` + direct `widget.render()` pattern**: The new tests use this pattern (matching the task specification) rather than `TestBackend` + `Terminal` pattern used by most existing tests. Both patterns are valid; `Buffer::empty` makes out-of-bounds panics explicit, which is exactly what makes these tests effective as overflow guards.

2. **Reused existing `buffer_to_string` helper**: The helper already existed at line 1720. No duplication needed.

3. **`test_min_height_arithmetic` required no update**: The existing test already accounts for `button_spacer` as a separate variable (line 1338), matching the 13-slot layout. The total is still 29 and the test passes unchanged. No comment update was needed.

4. **No `chunks[10]` references needed updating**: Scanning the existing tests showed no test referenced `chunks[10]` as "rest" — the layout was already 13 slots with `chunks[12]` as rest.

### Testing Performed

- `cargo test -p fdemon-tui` — Passed (788 tests, including 5 new)
- `cargo clippy --workspace -- -D warnings` — Passed (no warnings)

### Risks/Limitations

1. **Small-height render tests**: Tests at heights 15-28 with `compact(false)` exercise a scenario prevented by Phase 1's compact guard in production. Ratatui's `Layout::vertical` clamps children to zero-height when area is insufficient, so these tests pass without panic even though the button may not be visually rendered. This is expected behavior — the fix guarantees no out-of-bounds write.
