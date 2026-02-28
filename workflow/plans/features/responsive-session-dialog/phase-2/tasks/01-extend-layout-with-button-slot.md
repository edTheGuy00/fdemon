## Task: Extend `calculate_fields_layout()` to Include Button Slot

**Objective**: Add the button spacer and button as explicit `Layout` constraints in `calculate_fields_layout()`, changing it from 11 to 13 slots. This is the foundational change that brings the button into Ratatui's layout system so it can never overflow the area bounds.

**Depends on**: None (Phase 1 must be complete)

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs`: Modify `calculate_fields_layout()` (lines 784-804), `render_common_fields()` (lines 813-824)

### Details

**Current `calculate_fields_layout()` (lines 784-804):**
```rust
fn calculate_fields_layout(area: Rect) -> [Rect; 11] {
    let chunks = Layout::vertical([
        Constraint::Length(1), // Spacer          [0]
        Constraint::Length(4), // Configuration   [1]
        Constraint::Length(1), // Spacer          [2]
        Constraint::Length(4), // Mode            [3]
        Constraint::Length(1), // Spacer          [4]
        Constraint::Length(4), // Flavor          [5]
        Constraint::Length(1), // Spacer          [6]
        Constraint::Length(4), // Entry Point     [7]
        Constraint::Length(1), // Spacer          [8]
        Constraint::Length(4), // Dart Defines    [9]
        Constraint::Min(0),    // Rest            [10]
    ])
    .split(area);
    // ...converts Rc<[Rect]> → [Rect; 11]
}
```

**New `calculate_fields_layout()`:**
```rust
fn calculate_fields_layout(area: Rect) -> [Rect; 13] {
    let chunks = Layout::vertical([
        Constraint::Length(1), // Spacer          [0]
        Constraint::Length(4), // Configuration   [1]
        Constraint::Length(1), // Spacer          [2]
        Constraint::Length(4), // Mode            [3]
        Constraint::Length(1), // Spacer          [4]
        Constraint::Length(4), // Flavor          [5]
        Constraint::Length(1), // Spacer          [6]
        Constraint::Length(4), // Entry Point     [7]
        Constraint::Length(1), // Spacer          [8]
        Constraint::Length(4), // Dart Defines    [9]
        Constraint::Length(1), // Button spacer   [10]
        Constraint::Length(3), // Launch button   [11]
        Constraint::Min(0),    // Rest            [12]
    ])
    .split(area);
    // ...converts Rc<[Rect]> → [Rect; 13]
}
```

The key addition is two new slots:
- `[10]`: `Length(1)` — the 1-row spacer between dart defines and the button (currently computed as `+1` in the manual `y` calculation)
- `[11]`: `Length(3)` — the 3-row button area (currently a manual `Rect { height: 3, ... }`)
- `[12]`: `Min(0)` — the remainder absorber (moved from index 10 to 12)

**Why this works**: When `area.height < 29`, Ratatui's constraint solver will proportionally shrink the `Length` slots and the `Min(0)` absorber gets 0. The button slot may collapse to 0 rows, making it invisible rather than overflowing — the same safe behavior that `render_compact()` already exhibits.

**Update `render_common_fields` signature (line 813-814):**
```rust
// Before:
fn render_common_fields(chunks: &[Rect; 11], ...)
// After:
fn render_common_fields(chunks: &[Rect; 13], ...)
```

The function body is unchanged — it only accesses indices `[1]`, `[3]`, `[5]`, `[7]`, `[9]` which are the same in both layouts.

**Update the array conversion**: The `chunks` variable from `Layout::split()` returns `Rc<[Rect]>`. The existing code converts this to a fixed-size array. Update the conversion to produce `[Rect; 13]` instead of `[Rect; 11]`. Check whether this uses `try_into().unwrap()` or manual indexing and update accordingly.

### Acceptance Criteria

1. `calculate_fields_layout()` returns `[Rect; 13]` with button spacer at `[10]` and button at `[11]`
2. `render_common_fields` accepts `&[Rect; 13]` — body unchanged, only signature updated
3. Field indices `[0]`-`[9]` produce identical `Rect` values as before (same constraints in same order)
4. `chunks[11]` has `height == 3` when `area.height >= 29`
5. `chunks[11]` has `height == 0` when `area.height` is very small (Ratatui collapse behavior)
6. `LaunchContext::min_height()` remains `29` (no change — the arithmetic is: 5 spacers×1 + 5 fields×4 + 1 button_spacer + 3 button = 29)
7. `cargo check -p fdemon-tui` passes
8. `cargo test -p fdemon-tui` passes — existing tests remain green

### Testing

No new tests in this task — the function signature change will cause compile errors if any caller is missed, which serves as a built-in verification. Task 03 adds comprehensive tests for the new layout.

Verify with:
- `cargo check -p fdemon-tui` — confirms all callers updated
- `cargo test -p fdemon-tui` — confirms no behavioral regression

### Notes

- The total fixed rows remain 29: `1+4+1+4+1+4+1+4+1+4+1+3 = 29`. The `Min(0)` absorber adds 0 fixed rows. So `min_height()` stays at 29 — no change needed.
- The `Rc<[Rect]>` to `[Rect; N]` conversion pattern: check if the codebase uses `chunks.as_ref().try_into().unwrap()` or manual `[chunks[0], chunks[1], ...]`. The latter requires adding two more elements.
- Callers of `calculate_fields_layout()` are: `LaunchContext::render()` (line 858), `LaunchContextWithDevice::render_full()` (line 934), and the test module. All three need to compile with the new return type.
- The existing `test_min_height_arithmetic` test (line 1330) should still pass since the arithmetic is unchanged: `spacer(1) + config(4) + spacer(1) + mode(4) + spacer(1) + flavor(4) + spacer(1) + entry(4) + spacer(1) + dart_defines(4) + spacer(1) + button(3) = 29`.
