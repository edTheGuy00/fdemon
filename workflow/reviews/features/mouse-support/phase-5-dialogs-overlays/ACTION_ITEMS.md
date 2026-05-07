# Action Items: Mouse Support — Phase 5 (Dialogs & Overlays)

**Review Date:** 2026-05-06
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 2 critical, 5 major, 13 minor

---

## Critical Issues (Must Fix)

### 1. Modal-precedence leak: clicks outside a modal hit underlying base UI

- **Source:** `logic_reasoning_checker`, `risks_tradeoffs_analyzer`
- **Files:**
  - `crates/fdemon-tui/src/render/mod.rs:103-340`
  - `crates/fdemon-app/src/handler/mouse/{confirm_dialog,new_session,tag_filter,settings}.rs`
- **Problem:** `view()` registers `MainHeader`/`LogView` z=0 regions before the modal `match` block. Per-mode dispatchers call `regions.hit_test(...)` without a z-filter or modal-rect filter. A click outside the modal's z=1 rects but on a z=0 base region (e.g., the `[r]` HotReload bracket at y=1) returns the base-UI message instead of being absorbed by the modal.
- **Required Action:** Add a `z_index >= 1` filter (or modal-rect filter) inside each of `confirm_dialog::handle_press`, `new_session::handle_press`, `tag_filter::handle_press`, and `settings::handle_press`. Alternative: skip base-UI region recording at the renderer level when in a modal mode — preferred eventually but larger diff.
- **Acceptance:**
  - New test: when `NewSessionDialog` is open, `new_session::handle_press(state, header_x_of_r, 1, Left, NONE)` returns `None` (or a dialog message), not `Some(Message::HotReload)`. Repeat for `ConfirmDialog`, `TagFilter`, and `Settings`.
  - Existing acceptance criterion `TASKS.md:156` (click-precedence test) is now genuinely satisfied.

### 2. FuzzyModal scroll-offset underflow panic

- **Source:** `risks_tradeoffs_analyzer`
- **File:** `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs:233-235`
- **Problem:** `for screen_row in 0..(end - start)` underflows `usize` when `scroll_offset > filtered_indices.len()`. Triggered by typing a no-match query while previously scrolled. Debug: panic. Release: ~`usize::MAX` iterations, freeze.
- **Required Action:** Guard `if end <= start { return; }` before the loop, or use `start..end.max(start)`.
- **Acceptance:**
  - New test: open fuzzy modal with > visible_height items, scroll down past page 1, type a query that filters all rows out, render. Must not panic; must not produce regions.

---

## Major Issues (Should Fix)

### 3. Settings sub-modal click leak

- **Source:** `risks_tradeoffs_analyzer`
- **Files:**
  - `crates/fdemon-app/src/handler/mouse/settings.rs:38-50`
  - `crates/fdemon-tui/src/widgets/settings_panel/mod.rs:1365`
- **Problem:** `editing` is not set when `dart_defines_modal` or `extra_args_modal` is open, so settings click handler routes clicks under the modal to the underlying tab/row. Test gap: no integration test for "click while sub-modal open."
- **Suggested Action:** Add `if state.settings_view_state.has_modal_open() { return None; }` at top of `settings::handle_press`. Add equivalent in `new_session::handle_press` (dart-defines modal case).

### 4. Wrap-mode link badge y-position miscalculated

- **Source:** `risks_tradeoffs_analyzer`
- **File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs:1611-1659`
- **Problem:** When wrap mode is on and `col_offset >= visible_width`, the badge actually renders on a wrapped sub-row, but the recorded rect is at `(content_area.x + col_offset, rel_y)` — outside the content area. Click silently fails.
- **Suggested Action:** Compute `dy = col_offset / visible_width`, `dx = col_offset % visible_width`; shift `rel_y` by `dy` and use `dx` as the column. Or skip badge regions when wrap mode + overflow.

### 5. ConfirmDialog button centering math diverges from `Alignment::Center`

- **Source:** `code_quality_inspector`, `logic_reasoning_checker`
- **File:** `crates/fdemon-tui/src/widgets/confirm_dialog.rs:127-128`
- **Problem:** Manual `start_x` rounding may be off-by-one vs. ratatui's `Alignment::Center` when `width - total_width` is odd.
- **Suggested Action:** Make `Widget::render` delegate to `render_with_regions(area, buf, self, None)` — single source of truth eliminates drift. Alternatively replicate ratatui's rounding.

### 6. Settings layout constants duplicated between renderer and region recorder

- **Source:** `architecture_enforcer`, `code_quality_inspector`, `risks_tradeoffs_analyzer`
- **File:** `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`
- **Problem:** `tab_width=12`, `gap=1`, `Borders::ALL=1`, banner heights=4, section-skip rules independently re-derived in `render_with_regions` from values in `render_tab_bar`/`render_*_tab`. Drift would silently misalign click rects.
- **Suggested Action:** Extract module-level constants OR a private `compute_layout(area)` returning rects shared by both paths. Add a parity test that asserts a recorded `SettingsClickRow` rect's center contains the row's label text in the rendered buffer.

### 7. Tag-filter scroll-offset hand-rolled — may diverge from ratatui's `ListState`

- **Source:** `logic_reasoning_checker`, `risks_tradeoffs_analyzer`
- **File:** `crates/fdemon-tui/src/widgets/tag_filter.rs:296-311`
- **Problem:** Re-implementation of scroll arithmetic doesn't fully model ratatui's offset retention. Long lists with backward navigation can render selection mid-window while regions assume bottom-pinned.
- **Suggested Action:** Use a `Cell<usize>` write-back from the actual `ListState.offset()` (preferred). OR add a parity test rendering to `TestBackend`, scrolling, and asserting each recorded `abs_index` matches the rendered tag at that row.

---

## Minor Issues (Consider Fixing)

### 8. `tag_filter.rs:271` `unwrap_or(0)` collides with `[a]` region
Replace `footer_text.find("[n]").map(...).unwrap_or(0)` with a named constant `const N_ACTION_OFFSET: u16 = 9;`.

### 9. `handle_select_device_at` doesn't verify clamped index is a `Device(_)` not a header
After clamping, check `flat_list().get(clamped)` is `DeviceListItem::Device(_)`; else return `UpdateResult::none()`.

### 10. Save-logic duplicated between `handle_settings_save` and `handle_settings_save_and_close`
Extract a private `save_active_tab(state) -> Result<()>` helper.

### 11. Test names violate `test_<function>_<scenario>_<expected_result>` convention
Rename ~10 tests in `settings_handlers.rs::tests` to follow the project standard.

### 12. Stale comment in `render/tests.rs:87-92` predicting Phase 5 changes
Replace with current-state note.

### 13. "Add New Configuration" sentinel row not clickable
Append a sentinel region at `index = all_items.len()` in `register_setting_row_regions` (LaunchConfig branch).

### 14. FuzzyModal / LaunchContext re-render twice when ctx is present
Refactor to single-pass — visual render and region recording in one call.

### 15. Settings panel disk I/O on every render frame (LaunchConfig/VSCode tabs)
Cache `load_launch_configs` / `load_vscode_configs` in a render-hint `Cell` or thread the result.

### 16. `ConfirmDialog` warning text hardcoded for all dialogs
Move "All Flutter processes will be terminated." to an optional `warning: Option<String>` on `ConfirmDialogState`.

### 17. Cycle/increment stub functions mark dirty without changing values
Either implement, or stop calling `mark_dirty()` until implemented.

### 18. `pub mod clicks` exposes unnecessary public interface
Tighten to `pub(crate) mod clicks` to match sibling modules.

### 19. Right-click universal coverage missing as integration test
Add one test exercising right-click across all 7 UiModes.

### 20. Compact NewSessionDialog (40-69 wide × 20-21 tall) — mouse hole on narrow tmux panes
Either implement compact-vertical regions in Phase 6, or surface a UI hint when in this size band.

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] **Critical #1 fix verified:** New click-precedence tests for `NewSessionDialog`, `ConfirmDialog`, `TagFilter`, and `Settings` confirm modal handlers do NOT return base-UI messages for clicks outside modal rects.
- [ ] **Critical #2 fix verified:** No-match-while-scrolled regression test for fuzzy modal does not panic, produces 0 regions.
- [ ] **Major #3 fix verified:** `click while dart_defines_modal open` test confirms no tab-switch / row-select.
- [ ] All 5 Major fixes resolved or explicitly tracked as Phase 6 entry tickets with acceptance criteria.
- [ ] All Minor issues resolved or filed as follow-up items.
- [ ] Verification gates pass: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Manual mouse-only walkthrough on macOS terminal: open `NewSessionDialog`, click outside the dialog (on header `[r]`) → no hot reload fires; open `ConfirmDialog`, click outside the buttons → no underlying base action; open `Settings` → `dart_defines` modal, click underlying tab → no tab change.
