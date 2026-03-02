# Plan: Responsive New Session Dialog

## TL;DR

Make the New Session dialog truly responsive by basing compact/expanded decisions on actual available space (not just layout orientation), fixing button overflow in short terminals, and ensuring the selected device always stays visible when scrolling. Currently the compact/expanded logic is tied to horizontal vs vertical layout mode, which produces wrong results in common scenarios like side-panel (narrow but tall) and bottom-panel (wide but short) terminals.

---

## Background

The New Session dialog has two layout modes:
- **Horizontal** (width >= 70): Two-pane side-by-side (Target Selector | Launch Context)
- **Vertical** (width 40-69): Stacked (Target Selector above Launch Context)

The Launch Context section has two rendering variants:
- **Full/Expanded**: Stacked label+bordered-box fields (4 rows per field, ~25 rows total)
- **Compact/Inline**: Single-line `"Label: [value]"` fields (5 rows + spacer + 3-row button = 9 rows)

### Current Problems

**Problem 1: Compact/expanded tied to layout mode, not available space.**
Currently `compact(true)` is hardcoded for vertical layout and `compact(false)` for horizontal. This means:
- **Narrow-but-tall** terminal (e.g., side panel): Vertical layout triggers `compact(true)`, but there's plenty of vertical space to show expanded fields.
- **Wide-but-short** terminal (e.g., bottom panel): Horizontal layout triggers `compact(false)`, but the expanded fields (25 rows + 4 button) need ~29 rows, causing overflow when the content pane is only 10-15 rows.

**Problem 2: Launch button overflow.**
In `render_full()`, the button is positioned manually at `chunks[9].y + chunks[9].height + 1` relative to the last field. This position is calculated outside the layout system. When the content area is too short for all fields, the button renders beyond the dialog boundary.

**Problem 3: Scroll doesn't track selected item.**
The TEA handler calls `adjust_scroll(DEFAULT_ESTIMATED_VISIBLE_HEIGHT)` with a hardcoded value of `10`. The actual visible height varies by terminal size (could be 4 rows in a bottom panel or 20+ in a full terminal). When the real height < 10, the selected item scrolls out of view.

---

## Affected Modules

### Documentation
- `docs/CODE_STANDARDS.md` - Add "Responsive Layout Guidelines" section establishing project-wide standards

### TUI Layer (`crates/fdemon-tui/`)
- `src/widgets/new_session_dialog/mod.rs` - Layout mode logic, `render_horizontal()`, `render_vertical()`, dialog sizing
- `src/widgets/new_session_dialog/launch_context.rs` - `render_full()` button positioning, `render_compact()`, `calculate_fields_layout()`
- `src/widgets/new_session_dialog/target_selector.rs` - `render_full()`, `render_compact()` list area height
- `src/widgets/new_session_dialog/device_list.rs` - `ConnectedDeviceList::render()`, `BootableDeviceList::render()`, viewport slice

### App Layer (`crates/fdemon-app/`)
- `src/new_session_dialog/target_selector_state.rs` - `adjust_scroll()`, `last_known_visible_height` field
- `src/handler/new_session/target_selector.rs` - `handle_device_up/down()`, `DEFAULT_ESTIMATED_VISIBLE_HEIGHT`

---

## Development Phases

### Phase 0: Document Responsive Layout Standards

**Goal**: Establish project-wide responsive layout guidelines in `docs/CODE_STANDARDS.md` before implementing fixes. These standards apply to all widgets, not just the New Session dialog. Other widgets can be updated to comply later; for now we codify the rules so all future work follows them.

#### Steps

1. **Add "Responsive Layout Guidelines" section to `docs/CODE_STANDARDS.md`**
   - Place it after the existing "Architectural Code Patterns" section.
   - Cover the following principles:

2. **Principle: Decide layout variant based on available space, not orientation**
   - Compact vs expanded rendering must be driven by the actual dimensions of the area passed to the widget, not by whether the parent chose a horizontal or vertical layout mode.
   - Document the anti-pattern: hardcoding `compact(true)` in vertical and `compact(false)` in horizontal paths.
   - Document the correct pattern: measure `area.height` (or `area.width`) and compare against named threshold constants.

3. **Principle: All content must fit within the allocated area**
   - Every element rendered by a widget must fall within the `Rect` passed to its `render()` method.
   - Never manually compute positions that could exceed the area bounds. Use Ratatui's layout system (`Layout::vertical` / `Layout::horizontal`) to allocate space, and let `Min(0)` absorb overflow.
   - If content cannot fit, degrade gracefully (switch to compact mode, hide non-essential elements, show a "too small" message).

4. **Principle: Scrollable lists must keep the selected item visible**
   - Any scrollable list that tracks a `selected_index` and `scroll_offset` must ensure the selected item is within the visible viewport.
   - Hardcoded viewport height estimates are a code smell. Prefer feeding actual render-time height back to the state layer (via `Cell<usize>` or a `RenderHints` struct) so handlers can make accurate scroll adjustments.
   - As a safety net, the renderer should clamp scroll state so the selected item is visible even if the handler's estimate was off.

5. **Principle: Use named constants for layout thresholds**
   - All layout breakpoints (e.g., minimum heights for expanded mode, width thresholds for abbreviated labels) must be named constants with doc comments explaining the rationale.
   - Group related thresholds together near the widget they control.

6. **Principle: Add hysteresis at layout breakpoints**
   - When a widget switches between two modes at a size threshold, use a small buffer (2-3 rows/columns) to prevent flickering during resize. For example, switch to expanded at height >= 30, back to compact at height <= 26.

7. **Anti-pattern examples to include**
   - Hardcoding `compact(orientation == Vertical)` instead of checking available height
   - Manual `Rect` construction for elements that could overflow bounds
   - Using a hardcoded `DEFAULT_VISIBLE_HEIGHT = 10` for scroll calculations

**Milestone**: `docs/CODE_STANDARDS.md` contains a "Responsive Layout Guidelines" section that serves as the reference for this work and all future layout work.

---

### Phase 1: Space-Aware Compact/Expanded Decision

**Goal**: Decouple the compact/expanded decision from layout orientation. Use actual available vertical space to choose the right rendering mode for the Launch Context section.

#### Steps

1. **Introduce height thresholds for compact/expanded decision**
   - In `mod.rs`, define constants for the minimum height that the Launch Context content area needs to render in full/expanded mode. The expanded mode needs 5 fields x 4 rows + 4 spacers + 4 button rows = ~29 rows. A safe threshold: `MIN_EXPANDED_LAUNCH_HEIGHT = 28`.
   - Add a `MIN_COMPACT_LAUNCH_HEIGHT = 10` constant for the compact mode minimum (5 inline fields + spacer + 3 button = 9 rows, plus 2 for border = 11).

2. **Compute available height and select mode dynamically**
   - In `render_horizontal()`: After splitting `chunks[2]` (main content pane) with `render_panes()`, calculate the right pane's actual height. If height < `MIN_EXPANDED_LAUNCH_HEIGHT`, use `compact(true)` for Launch Context even in horizontal mode.
   - In `render_vertical()`: After computing `chunks[4]` (Launch Context area), check its height. If height >= `MIN_EXPANDED_LAUNCH_HEIGHT`, use `compact(false)` for Launch Context even in vertical mode. The Target Selector compact mode should also become height-aware: if `chunks[2]` has enough height (>= 8), use full tab bar and footer; otherwise use compact.

3. **Pass available height through `render_panes()`**
   - Modify `render_panes()` to accept the available area height and pass the appropriate `compact` flag to `LaunchContextWithDevice` based on height thresholds rather than hardcoding `compact(false)`.

**Milestone**: The Launch Context always shows the right variant: expanded when space is available (even in vertical layout), compact when space is tight (even in horizontal layout).

---

### Phase 2: Fix Launch Button Overflow

**Goal**: Ensure the Launch button never renders outside the dialog bounds, regardless of available space.

#### Steps

1. **Include button in `calculate_fields_layout()` layout system**
   - Currently `render_full()` manually computes the button `Rect` outside the layout. Change `calculate_fields_layout()` to include the button as a layout slot:
     ```
     [spacer, config, spacer, mode, spacer, flavor, spacer, entry, spacer, dart_defines, spacer, button(3), Min(0)]
     ```
   - This ensures Ratatui's layout engine clips the button to available space instead of overflowing.

2. **Add overflow guard for manual positioning fallback**
   - If keeping the manual calculation for any reason, add a bounds check:
     ```rust
     let button_y = chunks[9].y + chunks[9].height + 1;
     let max_y = area.y + area.height;
     if button_y + 3 <= max_y {
         // render button
     }
     ```
   - This prevents the button from rendering below the dialog.

3. **Auto-switch to compact if expanded overflows**
   - In both `render_horizontal()` and `render_vertical()`: Before rendering Launch Context, check if the available height can fit the expanded layout (29 rows). If not, automatically switch to compact mode. This is the same logic from Phase 1, which inherently solves the overflow since compact mode uses the layout system for button placement.

**Milestone**: The LAUNCH INSTANCE button is always visible within the dialog bounds at any terminal size.

---

### Phase 3: Fix Scroll-to-Selected in Target Selector

**Goal**: Ensure the selected device is always visible when scrolling through the device list, regardless of actual terminal height.

#### Steps

1. **Store last-known visible height in `TargetSelectorState`**
   - Add a `last_known_visible_height: usize` field to `TargetSelectorState` (default: `DEFAULT_ESTIMATED_VISIBLE_HEIGHT`).
   - This field acts as a feedback channel from the render layer to the state layer, updated each frame.

2. **Update visible height from the renderer**
   - In `ConnectedDeviceList::render()` and `BootableDeviceList::render()`, after computing `visible_height = area.height as usize`, write this value back to `TargetSelectorState.last_known_visible_height`.
   - **TEA consideration**: Since the renderer currently receives `&TargetSelectorState` (immutable), this needs either:
     - **Option A**: Store visible height in a `Cell<usize>` or `AtomicUsize` inside `TargetSelectorState` for interior mutability (minimal TEA violation).
     - **Option B**: Pass `&mut TargetSelectorState` to the device list widgets (breaks Ratatui widget convention of `&self` render).
     - **Option C** (Recommended): Use a separate shared `RenderHints` struct (containing `target_list_visible_height: Cell<usize>`) passed alongside state. The dialog widget writes to it during render; the handler reads it on next key event.

3. **Use actual visible height in handler**
   - In `handle_device_up()` and `handle_device_down()`, read `state.render_hints.target_list_visible_height` (or `state.target_selector.last_known_visible_height`) instead of `DEFAULT_ESTIMATED_VISIBLE_HEIGHT`.
   - Keep `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` as fallback for the first frame before any render has occurred.

4. **Add render-time scroll correction**
   - As additional safety, in `ConnectedDeviceList::render()` and `BootableDeviceList::render()`, after computing the visible slice, clamp `scroll_offset` so that `selected_index` is within `[start, end)`. This provides render-time correction in case the handler's estimate was slightly off.
   - Note: This needs the renderer to be able to mutate `scroll_offset`. Use the same interior mutability approach from step 2.

**Milestone**: The selected device is always visible in the list, regardless of terminal dimensions. No more "selected item scrolled off-screen" when height is small.

---

## Edge Cases & Risks

### Terminal Resize Mid-Dialog
- **Risk:** Terminal resize between handler and render could cause one frame of misalignment.
- **Mitigation:** Render-time scroll correction (Phase 3, step 4) catches this. The next frame will be correct.

### Very Small Terminals
- **Risk:** Below minimum thresholds, nothing fits.
- **Mitigation:** Existing `TooSmall` layout mode handles this. No changes needed for < 40x20.

### TEA Pattern Purity
- **Risk:** Phase 3 introduces render-to-state feedback, which technically violates TEA's unidirectional data flow.
- **Mitigation:** Use `Cell<usize>` interior mutability for a single numeric hint value. This is a pragmatic concession common in TUI frameworks where the renderer discovers layout-dependent information. The value is only a hint to improve scroll accuracy; the handler still works correctly (just less precisely) without it.

### Transition Between Compact and Expanded
- **Risk:** When resizing the terminal across the threshold, the Launch Context could flicker between modes.
- **Mitigation:** Add hysteresis of 2-3 rows: switch to expanded at height >= 30, but only switch back to compact at height <= 26. This prevents rapid toggling at the boundary.

---

## Success Criteria

### Phase 0 Complete When:
- [ ] `docs/CODE_STANDARDS.md` has a "Responsive Layout Guidelines" section
- [ ] All 5 principles are documented with anti-pattern examples
- [ ] Guidelines are general enough to apply to any widget, not just the New Session dialog

### Phase 1 Complete When:
- [ ] Narrow-but-tall terminal shows expanded Launch Context fields
- [ ] Wide-but-short terminal shows compact Launch Context fields
- [ ] Existing horizontal/vertical layout switching still works correctly
- [ ] Unit tests for height-based compact/expanded decision
- [ ] All existing tests pass

### Phase 2 Complete When:
- [ ] Launch button never renders outside dialog bounds at any terminal size
- [ ] Button is included in the layout system (not manually positioned)
- [ ] Compact mode auto-activates when expanded would overflow
- [ ] Visual verification at terminal heights 20, 25, 30, 40

### Phase 3 Complete When:
- [ ] Selected device is always visible when scrolling with arrow keys
- [ ] Works correctly at all terminal heights (tested at 20, 25, 40, 80 rows)
- [ ] First frame uses reasonable default, subsequent frames use actual height
- [ ] Scroll indicators (arrows) still display correctly
- [ ] All existing target selector tests pass + new scroll visibility tests

---

## Future Enhancements

- **Adaptive Target Selector split**: In vertical mode, the Target Selector currently gets a fixed `Percentage(45)`. This could be made dynamic based on the number of devices.
- **Animated transitions**: Smooth transition when switching between compact and expanded modes during resize.
- **Render-time layout metrics**: Extend the `RenderHints` pattern to other widgets that need layout feedback (e.g., log view scroll).
