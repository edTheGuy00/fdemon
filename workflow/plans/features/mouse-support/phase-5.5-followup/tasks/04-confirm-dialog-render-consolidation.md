# Task 04: ConfirmDialog Render Consolidation + Optional Warning Field

## Goal

Eliminate the divergence between `ConfirmDialog::Widget::render` and `render_with_regions` (Major #5) by making `Widget::render` delegate to `render_with_regions(_, _, _, None)`. Move the hardcoded "All Flutter processes will be terminated." text into an optional `warning: Option<String>` field on `ConfirmDialogState` (Minor #16).

## Background

`crates/fdemon-tui/src/widgets/confirm_dialog.rs:127-128` computes button positions via:
```rust
let start_x = button_row.x + ((button_row.width as usize).saturating_sub(total_width) / 2) as u16;
```
while the actual render uses `Paragraph::new(...).alignment(Alignment::Center)`, which rounds differently when `(button_row.width - total_width)` is odd. Region rects are off-by-one from the painted text in the odd case. A click on the rightmost cell of a button misses; a click one cell left of the visual start false-hits.

The cleanest fix is to make the rendering path *use the same arithmetic* — i.e., have `Widget::render` delegate to `render_with_regions` (passing `None` for ctx) so the hand-computed `start_x` is the single source of truth, and `Alignment::Center` is replaced with `Paragraph::new(spans)` rendered at the computed start_x via a `Buffer.set_line(...)` or a sub-rect.

Additionally, both `Widget::render` (lines 200-267) and `render_with_regions` (lines 85-88) hardcode "All Flutter processes will be terminated." for every confirm dialog, including the Settings unsaved-changes dialog where the text is misleading.

## Files

**Modify:**
- `crates/fdemon-tui/src/widgets/confirm_dialog.rs` — consolidate rendering, drop hardcoded warning
- `crates/fdemon-app/src/confirm_dialog.rs` — add `warning: Option<String>` field; update `quit_confirmation`/`unsaved_settings` constructors

**Read (reference):**
- `crates/fdemon-app/src/state.rs` — confirm dialog usage sites (read-only)

## Plan

1. **Add `warning: Option<String>` to `ConfirmDialogState`** in `crates/fdemon-app/src/confirm_dialog.rs`:
   ```rust
   pub struct ConfirmDialogState {
       pub message: String,
       pub options: Vec<(String, Message)>,
       pub warning: Option<String>,  // NEW
       // ... other fields
   }
   ```
   Update existing constructors:
   - `quit_confirmation(...)` sets `warning: Some("All Flutter processes will be terminated.".into())`.
   - Settings unsaved-changes constructor (search for callers; likely in `handler/settings_handlers.rs` or `handler/keys.rs`) sets `warning: None`.
   - Any other call sites: default to `None` unless the existing hardcoded text was meaningful.

2. **Update `widgets/confirm_dialog.rs`** to read `state.warning` instead of hardcoding the string. Around line 85-88 in `render_with_regions` and line 230 (or wherever) in `Widget::render`:
   ```rust
   if let Some(warning) = &state.warning {
       // Render warning line styled in red/yellow as today.
   }
   ```
   The warning row (currently always rendered) becomes conditionally rendered. The modal height `modal_height = 9` was sized assuming the warning is always present — verify it still fits when `warning: None`. If the layout shifts, keep `modal_height = 9` and leave the warning row blank when `None`, OR shrink `modal_height` to 8 when `None`. Choose the simpler option that doesn't require recomputing every layout constant.

3. **Consolidate `Widget::render` and `render_with_regions`** so the visual rendering is the SAME code path:

   **Option A (preferred):** Make `Widget::render` call `render_with_regions(area, buf, &self, None)`:
   ```rust
   impl Widget for ConfirmDialog<'_> {
       fn render(self, area: Rect, buf: &mut Buffer) {
           render_with_regions(area, buf, self, None);
       }
   }
   ```
   Then *delete* the duplicated layout-and-paint code in the existing `Widget::render` body. `render_with_regions` becomes the single rendering implementation.

   **Option B (fallback):** Extract a private `render_inner(state, area, buf, ctx: Option<&mut MouseCtx>)` that both `Widget::render` and `render_with_regions` call. Use this if Option A turns out to require complex lifetime juggling.

4. **Fix the centering math** so that `start_x` exactly matches the position where the centered `Paragraph` would paint. Two acceptable ways:

   - **(a)** Replace `Paragraph::new(line).alignment(Alignment::Center).render(button_row, buf)` with manual painting at `start_x` using `Buffer::set_line` or `set_string` calls. This makes `start_x` definitive — the regions and visuals share one calculation.
   - **(b)** Use ratatui's actual `Alignment::Center` formula in `start_x`: it rounds `(width - line_width)` toward zero in some versions. Test by rendering a 3-button dialog with `(width - total_width)` odd (e.g., width 50, total 25) and asserting the buffer's first non-empty cell on the button row matches `start_x`.

   Prefer (a) — eliminates the parity dependency entirely.

5. **Add tests** in `confirm_dialog.rs::tests`:

   - `widget_render_delegates_to_render_with_regions_byte_identical_visual` — render a 2-button quit dialog via `Widget::render` and via `render_with_regions(_, _, _, None)`, assert buffers byte-equal.
   - `render_with_regions_three_button_centering_alignment_odd_width` — 3-button dialog with `total_width` causing odd offset; assert each button rect's center column contains the rendered button's first character.
   - `render_with_regions_warning_some_renders_warning_line` — `warning: Some("X".into())` produces the warning row.
   - `render_with_regions_warning_none_omits_warning_line` — `warning: None` does not render any warning text.
   - `quit_confirmation_state_has_warning_set` — `ConfirmDialogState::quit_confirmation(...)` returns a state with `warning = Some("All Flutter processes will be terminated.")`.
   - `unsaved_settings_state_has_no_warning` — settings unsaved-changes dialog has `warning = None`.

6. **Update existing tests** that asserted on the warning text — they now need to construct the state with the explicit `warning` field.

7. **Quality gates:**
   ```bash
   cargo test -p fdemon-tui confirm_dialog
   cargo test -p fdemon-app
   cargo test --workspace
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

## Acceptance Criteria

- [ ] `ConfirmDialogState` has a `warning: Option<String>` field.
- [ ] `quit_confirmation` constructor sets `warning = Some("All Flutter processes will be terminated.")`.
- [ ] All other constructors set `warning = None` (or an explicit warning string if domain-relevant).
- [ ] `Widget::render` delegates to `render_with_regions(area, buf, self, None)` — no duplicate layout/paint code.
- [ ] Button-rect math matches the painted text byte-for-byte across all `(width - total_width)` parities, verified by test.
- [ ] All 6 new tests pass; existing tests updated to construct dialogs with `warning` field.
- [ ] Quality gates pass.

## Notes

- This task widens `ConfirmDialogState` (a public type in `fdemon-app`). Any external callers (binary crate, other tests) must be updated. Search for `ConfirmDialogState {` and `ConfirmDialogState::new` in the workspace.
- T01 (modal-precedence) does NOT modify `confirm_dialog.rs` (widget) or `fdemon-app/src/confirm_dialog.rs` (state). T04 owns both.
- Do NOT change the `options: Vec<(String, Message)>` shape — Phase 5's action-coupled buttons rely on it.
- If callers construct `ConfirmDialogState` via struct-literal syntax `ConfirmDialogState { ... }` (not via constructors), use a `#[non_exhaustive]` attribute or provide a `..Default::default()` fallback. Decide based on what the codebase currently does.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/confirm_dialog.rs` | Added `warning: Option<String>` field to `ConfirmDialogState`; updated `new()` to set `warning: None`; added `with_warning()` builder method; updated `quit_confirmation()` to set `warning: Some("All Flutter processes will be terminated.")` |
| `crates/fdemon-tui/src/widgets/confirm_dialog.rs` | Consolidated `Widget::render` to delegate to `render_with_regions(area, buf, self, None)`; deleted duplicated layout/paint code; updated warning line to read from `state.warning` (conditional render); replaced button row `Paragraph::Alignment::Center` with `Buffer::set_line` at manually computed `start_x`; added 6 new tests; updated 1 existing test (`test_confirm_dialog_rendering`) |

### Notable Decisions/Tradeoffs

1. **Widget::render delegation (Option A)**: `Widget::render` now simply calls `render_with_regions(area, buf, self, None)`. The full duplicated layout-and-paint body was deleted. This is clean with no lifetime juggling needed.

2. **Buffer::set_line over Alignment::Center**: The button row now uses `buf.set_line(start_x, ...)` with the same integer-division `start_x` used for click region rects. This eliminates any parity mismatch between painted text and region boundaries when `(width - total_width)` is odd.

3. **warning: None default via `new()`**: The `new()` constructor sets `warning: None` by default. The `with_warning()` builder allows callers who need a warning to set it fluently. No `#[non_exhaustive]` was needed because all construction sites use the constructor functions, not struct-literal syntax.

4. **modal_height = 9 unchanged**: The warning row is always allocated in the layout (keeping `modal_height = 9`), and simply renders blank when `warning` is `None`. This avoids recomputing all layout constants.

### Testing Performed

- `cargo test -p fdemon-tui confirm_dialog` — Passed (24/24 tests)
- `cargo test -p fdemon-app` — Passed (2116 tests)
- `cargo test --workspace` — Passed (all test suites)
- `cargo fmt --all -- --check` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **No off-by-one regression in centering**: The new `Buffer::set_line` approach makes the position authoritative. The test `render_with_regions_three_button_centering_alignment_odd_width` verifies that region rects align with the `[` character in the buffer, confirming parity correctness.
