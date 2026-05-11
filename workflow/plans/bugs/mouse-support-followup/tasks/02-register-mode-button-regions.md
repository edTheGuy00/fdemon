# Task 02: Register per-button click regions for the Mode selector

**File:** `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs`
**Depends on:** Task 01 (uses `Message::NewSessionDialogSetMode` route)
**Wave:** 1 (Worktree A, sequential after Task 01)

## Background

`ModeSelector::render` (lines 175-248) computes three button rects locally via
`Layout::horizontal([Constraint::Ratio(1, 3); 3]).spacing(1).split(button_row_rect)`
but never exports them. The row-level `register_full_layout_regions` (lines
1190-1217) and `register_compact_layout_regions` (lines 1223-1252) only
register one `MouseRect` per field row emitting `NewSessionDialogFocusField`.

## What to do

1. Add a private helper near the existing region-registration helpers:

   ```rust
   /// Register click regions for the three Mode buttons.
   ///
   /// `mode_row` is the rect allocated to the entire Mode field — index `3`
   /// in the expanded layout, index `1` in the compact layout. The button
   /// row lives inside `mode_row` according to `ModeSelector::render`:
   ///
   /// - Expanded: label is row 0 (1 row), buttons are rows 1-3 (3 rows).
   /// - Compact: only 1 row total → no button row exists, no registration.
   ///
   /// The three button sub-rects use the same `Layout::horizontal` split as
   /// `ModeSelector::render`. Registered at `z_index = 2` so they win the
   /// hit-test over the row-level `FocusField` region at `z = 1`.
   fn register_mode_button_regions(mode_row: Rect, ctx: &mut crate::widgets::MouseCtx<'_>) {
       // Skip when the row is too short to contain the button row.
       if mode_row.height < 4 {
           return;
       }
       let chunks = Layout::vertical([
           Constraint::Length(1), // Label
           Constraint::Length(3), // Buttons
       ])
       .split(mode_row);
       let button_areas = Layout::horizontal([
           Constraint::Ratio(1, 3),
           Constraint::Ratio(1, 3),
           Constraint::Ratio(1, 3),
       ])
       .spacing(1)
       .split(chunks[1]);

       let modes = [
           FlutterMode::Debug,
           FlutterMode::Profile,
           FlutterMode::Release,
       ];
       for (i, mode) in modes.iter().enumerate() {
           let r = button_areas[i];
           if r.width > 0 && r.height > 0 {
               ctx.click_at_z(
                   MouseRect::new(r.x, r.y, r.width, r.height),
                   MouseAction::emit(Message::NewSessionDialogSetMode { mode: *mode }),
                   2,
               );
           }
       }
   }
   ```

2. In `register_full_layout_regions` (lines 1190-1217), after the `for` loop
   that registers field rows, add:

   ```rust
   register_mode_button_regions(chunks[3], ctx);
   ```

3. The compact layout reserves only 1 row for the Mode field
   (`launch_context.rs:1281-1287` — `Constraint::Length(1)`). `ModeSelector`
   in compact mode does not render the bordered buttons, so no per-button
   regions need to be registered. The early-exit in
   `register_mode_button_regions` handles this case.

   Document this with a comment in `register_compact_layout_regions` near
   the Mode field entry:

   ```rust
   // Compact layout reserves 1 row for Mode — the bordered buttons are
   // suppressed by ModeSelector's compact path, so per-button regions are
   // not registered (the row-level FocusField region above still applies).
   ```

## Verification

- `cargo check -p fdemon-tui` compiles.
- `cargo test -p fdemon-tui -- widgets::new_session_dialog::launch_context`
  passes.
- Add unit tests in the same file (or its `tests` sub-module):
  - `test_mode_button_regions_registered_in_expanded_layout` — render the
    widget with a `MouseRegionsBuilder`, perform `hit_test` at the centre of
    each button, assert `NewSessionDialogSetMode { mode: Debug/Profile/Release }`.
  - `test_mode_button_regions_z_index_wins_over_focus_field` — confirm a
    click inside a button rect resolves to `SetMode`, not `FocusField`,
    because z=2 beats z=1.
  - `test_mode_button_regions_skipped_in_compact_layout` — render compact,
    confirm the only Mode-row region emits `FocusField` (no `SetMode`
    entries present).
  - `test_mode_button_regions_label_row_still_focuses_field` — click at the
    label band above the buttons → `FocusField`.
- `cargo clippy -p fdemon-tui -- -D warnings` passes.
- Manual: launch fdemon in a terminal that supports mouse, open the New
  Session dialog, click each mode button — selection visually changes.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a4646d8443e6618f2

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | Added `register_mode_button_regions()` private helper; called it from `register_full_layout_regions()` after the field-row loop; added comment in `register_compact_layout_regions()`; updated existing `launch_context_full_mode_registers_all_fields_and_launch_button_at_z1` test (now expects 9+ regions, z=1 or z=2); added 4 new unit tests |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Updated `render_with_regions_integration_horizontal_layout_counts_regions` to expect exactly 3 z=2 regions (mode buttons) instead of 0 |

### Notable Decisions/Tradeoffs

1. **z=2 for mode buttons**: Registered at z_index=2 per task spec, so hit_test selects them over the row-level FocusField region at z=1. The label row (y offset 0 within mode_row) is not covered by any SetMode region, so label clicks still resolve to FocusField via z=1.
2. **Early exit via height < 4**: The guard `if mode_row.height < 4` handles both compact layout (height=1) and any other degenerate small-rect scenarios without registering zero-area regions.
3. **Integration test update**: The existing count test in `mod.rs` expected 0 z=2 regions; updated to expect exactly 3 (Debug/Profile/Release buttons) as a regression guard.
4. **Unused helper removal**: An `is_set_mode` helper function was created but ultimately not needed (tests used inline match closures). Removed to satisfy `cargo clippy -D warnings`.

### Testing Performed

- `cargo check -p fdemon-tui` - Passed
- `cargo test -p fdemon-tui -- widgets::new_session_dialog::launch_context` - Passed (48 tests: 4 new + 44 existing)
- `cargo test --workspace` - Passed (all test suites: 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- `cargo fmt --all -- --check` - Passed

### Risks/Limitations

1. **Manual smoke test not performed**: The task calls for manual verification that clicking mode buttons in a real terminal updates the selection. This is not blocked — all automated tests pass and the implementation correctly follows `ModeSelector::render`'s layout math.
2. **3-button region count is hardcoded in integration test**: If mode button count changes (e.g., a new mode is added), `mod.rs` test would need updating too.
