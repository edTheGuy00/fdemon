## Task: Add DartDefines Field to Rendered Layout

**Objective**: Make the DartDefines field visible in both horizontal and compact layouts. Currently, `LaunchContextField` includes `DartDefines` in its navigation cycle but the TUI never renders it, creating a ghost field — the user tabs to an invisible field, sees no visual change, and can accidentally open the dart defines modal from nowhere.

**Depends on**: None

**Review Reference**: REVIEW.md #1 (Critical), ACTION_ITEMS.md #1

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs:709-727`: Add DartDefines slot to `calculate_fields_layout()` (currently 9 chunks, needs 11 — add spacer + DartDefines between Entry Point and Rest)
- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs:736-746`: Add `render_dart_defines_field()` call to `render_common_fields()`
- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs:878-927`: Add DartDefines row to compact mode layout
- Create `render_dart_defines_field()` helper function (and `render_dart_defines_inline()` for compact mode)

### Details

**Root cause**: The layout function `calculate_fields_layout()` allocates slots for Config, Mode, Flavor, and Entry Point, but skips DartDefines entirely. The rendering function `render_common_fields()` calls render helpers for those 4 fields but never renders DartDefines. The compact layout similarly omits it.

**Navigation flow with the bug**:
1. User presses Tab from Entry Point → state moves to `LaunchContextField::DartDefines`
2. TUI renders only Config, Mode, Flavor, Entry Point → no visual change, focus "disappears"
3. User presses Tab again → state moves to `LaunchContextField::Launch` → focus becomes visible again
4. User presses Enter on the ghost field → dart defines modal opens from nowhere

**Fix approach**:

1. **Full layout** — Update `calculate_fields_layout()` to include DartDefines:
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
           Constraint::Length(1), // Spacer          [8]  ← NEW
           Constraint::Length(4), // Dart Defines    [9]  ← NEW
           Constraint::Min(0),   // Rest            [10]
       ])
       .split(area);
   }
   ```

2. **Create `render_dart_defines_field()` helper** — Use the existing `ActionField` widget:
   ```rust
   fn render_dart_defines_field(area: Rect, buf: &mut Buffer, state: &LaunchContextState, is_pane_focused: bool) {
       let is_focused = is_pane_focused && state.focused_field == LaunchContextField::DartDefines;
       let is_disabled = !state.are_dart_defines_editable();
       let display = state.dart_defines_display();
       ActionField::new("DART DEFINES", &display)
           .focused(is_focused)
           .disabled(is_disabled)
           .render(area, buf);
   }
   ```

3. **Add to `render_common_fields()`** — Call the new helper for chunk[9]:
   ```rust
   render_dart_defines_field(chunks[9], buf, state, is_focused);
   ```

4. **Compact layout** — Add a DartDefines inline row (similar to Entry Point inline):
   ```rust
   // In compact layout constraints:
   Constraint::Length(1), // Dart Defines field (inline)
   ```
   Create `render_dart_defines_inline()` for single-line compact rendering.

5. **Update all callers** of `calculate_fields_layout()` to use the new 11-element array and update chunk indices for button area calculation (line 788: `chunks[7]` → `chunks[9]`).

**Existing infrastructure**:
- `ActionField` widget at line 251-345: Designed for exactly this purpose, renders label + value with focus/disabled styling
- `LaunchContextState::dart_defines_display()`: Returns formatted string like "3 defines"
- `LaunchContextState::are_dart_defines_editable()`: Returns false for VSCode configs (read-only)
- Handler at `navigation.rs:153-168`: Already handles Enter key on DartDefines field correctly

### Acceptance Criteria

1. DartDefines field is visible between Entry Point and Launch button in horizontal layout
2. DartDefines field is visible in compact layout as an inline row
3. Field shows label "DART DEFINES" and value from `dart_defines_display()`
4. Field shows focus styling (active border) when navigated to via keyboard
5. Field shows disabled styling when config is not editable (VSCode imported configs)
6. Pressing Enter on focused DartDefines field opens the dart defines modal
7. No ghost/invisible navigation states remain
8. `cargo check -p fdemon-tui` passes
9. `cargo clippy -p fdemon-tui -- -D warnings` passes

### Testing

- Uncomment the 4 commented-out DartDefines assertions at lines 512, 1109, 1281, 1748 (defer to Task 06 for thorough test updates)
- Verify existing tests pass with updated chunk array size
- Manually verify Tab navigation cycles through all fields visually

### Notes

- The `ActionField` widget is already fully implemented and tested (line 504 has standalone tests). This task wires it into the actual layout.
- The button area y-coordinate calculation (line 788) references `chunks[7]` — this must be updated to `chunks[9]` to account for the new DartDefines field.
- All three rendering paths need updating: `LaunchContext::render()`, `LaunchContextWithDevice::render_full()`, and `LaunchContextWithDevice::render_compact()`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | Updated `calculate_fields_layout()` from 9 chunks to 11 chunks, added `render_dart_defines_field()` and `render_dart_defines_inline()` helper functions, updated `render_common_fields()` to call DartDefines renderer, added DartDefines to compact layout, updated button area calculation from chunks[7] to chunks[9], updated `min_height()` from 21 to 26, uncommented 4 DartDefines test assertions, updated 5 test terminal heights from 25 to 30 to accommodate new field |

### Notable Decisions/Tradeoffs

1. **ActionField Widget Reuse**: Used the existing `ActionField` widget (designed for action fields like DartDefines and EntryPoint) rather than creating a new widget. This maintains consistency and leverages existing styling.
2. **Inline Compact Rendering**: Created `render_dart_defines_inline()` following the same pattern as `render_entry_inline()` to maintain visual consistency in compact mode.
3. **Test Height Updates**: Updated test terminal heights from 25 to 30 to accommodate the new field and ensure launch button is visible. This matches the new `min_height()` of 26.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-tui` - Passed
- `cargo test -p fdemon-tui --lib` - Passed (428 tests, 0 failures)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed

### Risks/Limitations

1. **UI Height Requirements**: The new minimum height of 26 rows (up from 21) means the dialog requires more vertical space. This should not be an issue on standard terminals but may affect very constrained environments.
2. **No Manual Testing**: While all unit tests pass, manual visual testing of Tab navigation through the DartDefines field was not performed in this session. The handler logic already exists and unit tests verify rendering, so the feature should work correctly.
