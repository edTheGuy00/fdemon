# Review: Mouse Support — Phase 5 (Dialogs & Overlays)

**Review Date:** 2026-05-06
**Branch:** `feat/mouse-support`
**Diff Range:** `fb2ecf1..bb2dc12` (11 commits, 29 files, +4633/-201 lines)
**Plan:** `workflow/plans/features/mouse-support/phase-5-dialogs-overlays/`
**Verdict:** ⚠️ **NEEDS WORK**

---

## Summary

Phase 5 wires clickable regions for `NewSessionDialog`, `ConfirmDialog`, `Settings`, the `TagFilter` overlay, and `LinkHighlight` badges. The work is large in scope and broadly well-implemented: per-frame region recording, the action-coupled confirm-dialog pattern, the Phase-4 double-click pattern reuse for Settings, and the test coverage (≈ 60 new tests) are all solid. Layer boundaries are clean (no new ratatui leak into `fdemon-app`, no internal deps in `fdemon-core`).

However, **modal precedence is logically broken** in a way that the existing tests do not catch. Several handlers also have edge cases that can crash the TUI or silently misroute clicks. The good news is most issues are small surgical fixes, not redesigns.

### Verdicts by Reviewer

| Agent | Verdict |
|---|---|
| `architecture_enforcer` | APPROVED WITH CONCERNS |
| `code_quality_inspector` | NEEDS WORK |
| `logic_reasoning_checker` | **REJECTED** (modal precedence) |
| `risks_tradeoffs_analyzer` | NEEDS WORK |
| `security_reviewer` | APPROVED |

Overall: **NEEDS WORK** — the modal precedence leak (Critical #1) is a real correctness bug that the v1 release cannot ship with. Fixes are bounded; re-review after addressing Critical findings.

---

## Critical Issues (Must Fix Before Ship)

### 1. Modal-precedence leak: clicks outside a modal hit underlying base UI

**Sources:** `logic_reasoning_checker` (Critical #1-3), `risks_tradeoffs_analyzer` (Major #1)
**Files:**
- `crates/fdemon-tui/src/render/mod.rs:103-340`
- `crates/fdemon-app/src/handler/mouse/confirm_dialog.rs:19-41`
- `crates/fdemon-app/src/handler/mouse/new_session.rs:32-64`
- `crates/fdemon-app/src/handler/mouse/tag_filter.rs:24-46`
- `crates/fdemon-app/src/handler/mouse/settings.rs:38-50`

**Problem:** `render::view()` registers `MainHeader`, `LogView`, and (for Settings mode) `SettingsPanel` regions at z=0 *before* the modal `match state.ui_mode` block runs. Per-mode dispatchers in `confirm_dialog.rs`, `new_session.rs`, `tag_filter.rs`, and `settings.rs` then call `regions.hit_test(x, y, button)` *without* any z-filter or modal-rect filter. `hit_test` returns the highest-z entry whose rect *contains* (x, y) — but if the click point lies outside the modal's z=1 rects, only the underlying z=0 base regions contain that point, so a base-UI message is returned.

**Concrete scenarios:**

1. `UiMode::ConfirmDialog` (50×9 modal). User clicks anywhere outside the two `[y] Yes` / `[n] No` button rects but on a header bracket (e.g., `[r]` at y=1, `[d]`, `[s]`, `[q]`) or on a session tab. **Result:** `Message::HotReload` (or other base-UI message) fires while the quit confirmation is still on screen — likely a hot reload during quit.
2. `UiMode::NewSessionDialog` (~80%×70% modal). User clicks the small "halo" outside the dialog body but on a header bracket. **Result:** base-UI message fires; the dialog acts non-modal.
3. `UiMode::Normal` with `tag_filter_visible == true`. User clicks a log row visible behind the centered tag-filter overlay. **Result:** `Message::ClickLogRow` fires through the overlay.
4. `UiMode::Settings`. The full-screen panel renders over the header visually, but the `MainHeader`'s `[r]` z=0 region is still in the registry at the same coordinates. `SettingsPanel` only registers tab regions at y≈4 and row regions further down. A click at y=1 (in the panel's title/header band) finds *only* the underlying `MainHeader`'s region.

**Acceptance criterion missed:** `TASKS.md:156` mandates a click-precedence test verifying that a click on `[r]` while `NewSessionDialog` is open is intercepted by the dialog's z=1 region. The integration test `phase5_modal_z1_region_wins_over_base_z0_region_at_same_cell` only proves that hit_test selects z=1 *when both regions overlap the click point* — not the realistic case where the modal does not cover the header.

**Required fix (pick one):**
- **(a) Per-mode dispatcher gate** (smallest diff): In each of the 4 modal dispatchers (`confirm_dialog`, `new_session`, `tag_filter`, `settings`) and any UiMode-specific handler that should be modal, replace `regions.hit_test(...)` with a filter that requires `entry.z_index >= 1` (or `>= 2` when a sub-modal is open). Document the convention: modal handlers ignore base-UI regions.
- **(b) Renderer-level gate** (cleaner, larger diff): When `state.ui_mode` is a modal mode, do not register base-UI regions at all. Skip the `MainHeader` and `LogView` ctx threads in those branches.

Option (a) is preferred for the v1 fix because it is local and testable. Option (b) is a Phase 6 follow-up.

**Verification test (must add):** Render `NewSessionDialog` at 100×40 (or 80×24) with the `MainHeader` ctx threaded; assert `new_session::handle_press(state, header_x_of_r, 1, Left, NONE)` returns `None` (or a dialog-specific message), not `Some(Message::HotReload)`. Repeat for `ConfirmDialog` and tag-filter overlay.

---

### 2. FuzzyModal scroll panic — integer underflow when query filters past current scroll

**Source:** `risks_tradeoffs_analyzer` (Major #4)
**File:** `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs:233-235`

**Problem:**
```rust
let start = modal.state.scroll_offset;
let end = (start + visible_height).min(modal.state.filtered_indices.len());
for screen_row in 0..(end - start) {
```
If `scroll_offset > filtered_indices.len()` — possible when the user types a query that filters all results out while previously scrolled — then `end = filtered_indices.len() < start`, and `end - start` underflows `usize`. In debug this panics; in release this wraps to a huge value and the loop iterates ~`usize::MAX` times.

**Required fix:** Either guard `if end <= start { return; }` before the loop, or use `start..end.max(start)`. Add a regression test that types a no-match query while `scroll_offset > 0`.

---

## Major Issues (Should Fix)

### 3. Settings sub-modal click leak — clicks fire on tab/row while dart-defines modal is open

**Source:** `risks_tradeoffs_analyzer` (Major #1)
**Files:**
- `crates/fdemon-app/src/handler/mouse/settings.rs:38-50`
- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs:1365` (no skip while sub-modal open)

**Problem:** `settings::handle_press` only gates on `state.settings_view_state.editing`. Opening `dart_defines_modal` or `extra_args_modal` does NOT set `editing = true`. Meanwhile `settings_panel::render_with_regions` continues to register tab + row regions at z=0 even when these sub-modals are open. A click that overlaps an underlying tab/row will switch tabs or change the selected row underneath the open modal.

**Required fix:** Add `if state.settings_view_state.has_modal_open() { return None; }` to the top of `settings::handle_press`. The helper exists at `handler/settings_dart_defines.rs:21`. Add an analogous guard in `new_session::handle_press` for the dart-defines sub-modal once that lands. Add a test that exercises "click while sub-modal open."

---

### 4. Wrap-mode link badge y-position is wrong

**Source:** `risks_tradeoffs_analyzer` (Major #3)
**File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs:1611-1659`

**Problem:** `BadgeAction.col_offset` is the absolute column position within the unwrapped `Line`. In wrap mode, when `col_offset >= visible_width`, the badge actually renders on a wrapped sub-row, not at `(content_area.x + col_offset, rel_y)`. The current code adds `col_offset` to `content_area.x` directly, so the rect lands outside the content area, gets clipped to width 0 or 1, and the click silently fails.

**Required fix:** Compute wrapped position via `dy = col_offset / visible_width; dx = col_offset % visible_width`, and shift `rel_y` by `dy`. Alternatively, document the limitation and skip badge regions explicitly when `col_offset >= visible_width` and wrap mode is on. Add a test rendering a wrapped log line whose badge falls past `visible_width`.

---

### 5. ConfirmDialog button centering math diverges from `Alignment::Center` when total width is odd

**Sources:** `code_quality_inspector` (Major #1), `logic_reasoning_checker` (Warning #9)
**File:** `crates/fdemon-tui/src/widgets/confirm_dialog.rs:127-128`

**Problem:** Region rects use:
```rust
let start_x = button_row.x + ((button_row.width as usize).saturating_sub(total_width) / 2) as u16;
```
Visual rendering uses `Paragraph::new(...).alignment(Alignment::Center)`, whose centering rounds differently. When `(button_row.width - total_width)` is odd, the recorded region rects are offset by one column from where the text is painted. A click on the rightmost cell of a button misses; a click one column left of the visual start false-hits.

**Required fix:** Render via the same path as the regions are computed, or compute `start_x` to match ratatui's actual centering rounding. The cleanest fix is to make `Widget::render` delegate to `render_with_regions(area, buf, self, None)` so the math has a single source of truth.

---

### 6. Settings layout constants duplicated between renderer and region recorder

**Sources:** `architecture_enforcer` (Warning), `code_quality_inspector` (Major #2), `risks_tradeoffs_analyzer` (Minor #8)
**Files:**
- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs:1389-1563` (region recorder)
- Same file, `render_tab_bar`, `render_header`, `render_user_prefs_info`, etc. (renderer)

**Problem:** `render_with_regions` re-derives `tab_width=12`, `gap=1`, `Borders::ALL=1`, banner height=4 (UserPrefs/VSCode tabs), section-skip logic, and inner-content offsets independently from the renderer's hardcoded values. If any of those change in `render_tab_bar`/`render_user_prefs_tab`/etc., click regions will silently misalign.

**Required fix:** Either extract module-level constants used by both paths, or expose a private `compute_layout(area: Rect) -> SettingsLayout` returning rects/positions for both call sites. Add a parity snapshot test that asserts a recorded `SettingsClickRow` rect's center contains the expected row's label text.

---

### 7. Tag-filter scroll-offset hand-rolled — may diverge from ratatui's `ListState`

**Sources:** `logic_reasoning_checker` (Major #5), `risks_tradeoffs_analyzer` (Major #2)
**File:** `crates/fdemon-tui/src/widgets/tag_filter.rs:296-311`

**Problem:** `compute_scroll_offset` re-implements scroll arithmetic; ratatui's `ListState` offset depends on prior offset state in a way the simplified function does not capture. Long tag lists with backward navigation can render the selection mid-window while the recorded `abs_index` assumes bottom-pinned. Test `render_with_regions_scrolled_indices_are_absolute` only asserts `max_index >= 25`, too weak to catch sub-row drift.

**Required fix:** Replace the reimplementation with a `Cell<usize>` write-back from the actual `ListState.offset()` (preferred). Or add a parity test that renders to `TestBackend`, scrolls, and asserts each recorded `abs_index` matches the visually rendered tag at that row.

---

## Minor Issues (Consider Fixing)

### 8. `tag_filter.rs:271` — `unwrap_or(0)` fallback would alias `[a]` and `[n]` if the search ever fails

**Source:** `code_quality_inspector` (Major #3)
**File:** `crates/fdemon-tui/src/widgets/tag_filter.rs:271`

`let n_offset = footer_text.find("[n]").map(|i| i as u16).unwrap_or(0);` — fallback collides with `[a]` at column 0. The string is local so failure cannot occur in practice, but the fallback masks the real intent. Replace with a named constant: `const N_ACTION_OFFSET: u16 = 9;`.

### 9. `handle_select_device_at` doesn't verify the clamped index is a `Device(_)`, not a header

**Source:** `security_reviewer` (Medium)
**File:** `crates/fdemon-app/src/handler/new_session/clicks.rs:28-31`

The flat list includes group headers. Renderer guards regions against headers, but the handler doesn't independently verify. A clamping path could land on a header. Add `flat_list().get(clamped).is_some_and(|item| matches!(item, DeviceListItem::Device(_)))` before emitting.

### 10. Save-logic duplicated between `handle_settings_save` and `handle_settings_save_and_close`

**Source:** `code_quality_inspector` (Minor #4)
**File:** `crates/fdemon-app/src/handler/settings_handlers.rs:158-198, 346-381`

Identical match-on-active-tab blocks in both functions. Extract a private `save_active_tab(state) -> Result<()>` helper.

### 11. Test names violate `test_<function>_<scenario>_<expected_result>` convention

**Source:** `code_quality_inspector` (Minor #8)
**File:** `crates/fdemon-app/src/handler/settings_handlers.rs:787-800` (and similar)

Names like `single_click_sets_selected_index_and_no_follow_up` lack the `test_<function>_` prefix mandated by `docs/CODE_STANDARDS.md`. Mass-rename for consistency.

### 12. Stale comment in `render/tests.rs` predicting Phase 5 changes that have now landed

**Source:** `code_quality_inspector` (Minor #9)
**File:** `crates/fdemon-tui/src/render/tests.rs:87-92`

Comment says "update this exact-count assertion when Phase 5 regions land" — Phase 5 has landed. Replace with a precise note.

### 13. "Add New Configuration" sentinel row not clickable

**Source:** `code_quality_inspector` (Minor #7)
**File:** `crates/fdemon-tui/src/widgets/settings_panel/mod.rs:1469-1486`

`get_item_count_for_tab` accounts for the "Add New" sentinel row, but `register_setting_row_regions` is called with the raw `launch_config_items(...)` output and skips the sentinel. Clicking "Add New Configuration" has no effect; only the keyboard path works. Append a sentinel region at `index = all_items.len()`.

### 14. FuzzyModal / LaunchContext re-render twice when ctx is present

**Sources:** `code_quality_inspector` (Minor #6), `risks_tradeoffs_analyzer` (Minor #7)
**Files:**
- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs:797-816, 869+`
- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs:1267, 1296`

`render_fuzzy_modal_overlay` renders once, then `fuzzy_modal_render_with_regions` re-renders the modal again. Same for launch_context. Idempotent but ~2x render cost on the hot path. Refactor to single-pass when convenient.

### 15. Settings panel I/O on every render frame (LaunchConfig/VSCode tabs)

**Source:** `risks_tradeoffs_analyzer` (Performance)
**File:** `crates/fdemon-tui/src/widgets/settings_panel/mod.rs:1469-1486`

`load_launch_configs(project_path)` and `load_vscode_configs(project_path)` are called both in `StatefulWidget::render` and again in `render_with_regions`. At 60fps on those tabs that is ~120 disk reads/sec. Cache the loaded configs in a render-hint `Cell` or thread the result from one call site to the other.

### 16. `ConfirmDialog` warning text hardcoded for all dialogs

**Source:** `security_reviewer` (Low)
**File:** `crates/fdemon-tui/src/widgets/confirm_dialog.rs:85-88`

"All Flutter processes will be terminated." is rendered for every confirm dialog including the Settings unsaved-changes dialog where it is misleading. Move to an optional `warning: Option<String>` field on `ConfirmDialogState`.

### 17. Stub functions in `settings_handlers.rs` mark dirty without changing values

**Source:** `code_quality_inspector` (Minor #5)
**File:** `crates/fdemon-app/src/handler/settings_handlers.rs:269-291`

`handle_settings_cycle_enum_next/_prev` and `handle_settings_increment` call `mark_dirty()` despite being no-op stubs. A user clicking a cycle button sees the form go dirty without anything changing. Pre-existing, but should be addressed since the file was touched.

### 18. `pub mod clicks` exposes unnecessary public interface

**Source:** `architecture_enforcer` (Suggestion)
**File:** `crates/fdemon-app/src/handler/new_session/mod.rs:11`

Sibling submodules use `mod`; `clicks` uses `pub mod`. Tighten to `pub(crate) mod clicks` to match the convention.

### 19. Right-click universal coverage not asserted by integration test

**Source:** `risks_tradeoffs_analyzer` (Testing Gap #8)

Each dispatcher has its own right-click no-op test. Add a single integration test that exercises right-click across all 7 UiModes to lock the v1 reservation in.

### 20. Compact NewSessionDialog (narrow tmux pane) — mouse-only walkthrough breaks

**Source:** `risks_tradeoffs_analyzer` (Documented #1)
**File:** `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` (compact path)

Task 09 deferred device-row regions in the compact-vertical layout to Phase 6. Concern: 40-69 wide × 20-21 tall is realistic for narrow tmux panes (a common Flutter dev workflow on laptops). Either fix in Phase 6 or surface a "Mouse not supported at this size — resize or use keyboard" hint.

---

## Strengths

These are non-trivially correct and worth keeping verbatim:

- **Action-coupled `ConfirmDialog`**: Buttons emit `state.confirm_dialog_state.options[i].1` rather than hard-coding `ConfirmQuit`/`CancelQuit`. Future confirm dialogs (unsaved settings, etc.) become clickable for free.
- **Settings double-click stamp**: Faithful Phase-4 pattern reuse, including stamp consumption on double-click and reset on tab change. Tests cover the third-click-no-refire edge case.
- **`DOUBLE_CLICK_WINDOW_MS` shared constant** in `handler/mod.rs` — single source of truth for log-view and settings double-click windows.
- **`link_highlight_state.is_active()` gate** on the badge-by-style detector reduces the false-positive surface to near-zero in v1.
- **Right-click reservation** consistent across all 7 mouse handlers (5 new + 2 existing).
- **Layer hygiene**: no new ratatui imports in `fdemon-app`, no `daemon` imports in `fdemon-tui` outside `dev-dependencies`, `fdemon-core` zero internal deps preserved.
- **Thorough test scaffolding**: ~60 new tests across handler/render/widget layers, plus 9 cross-cutting Phase-5 integration tests in Task 11.

---

## Documentation Freshness

No new modules, crates, or build steps were introduced; existing docs (`ARCHITECTURE.md`, `DEVELOPMENT.md`, `CODE_STANDARDS.md`) remain accurate. The z-index policy committed by Phase 5 (z=0 base, z=1 modal, z=2 sub-modal) is documented in `TASKS.md` notes; consider adding a brief paragraph to `docs/REVIEW_FOCUS.md` once Critical #1 is resolved.

Two minor cleanups worth doing once issues land:
- Update task specs that reference `confirm_dialog_state.actions[i].1` — actual field is `options`.
- Update the Phase 5 plan PR description to flag the modal-precedence fix as a follow-up commit.

---

## Re-review Required

Re-review after addressing **Critical #1** (modal precedence) and **Critical #2** (FuzzyModal panic). Major #3–#7 are tracked separately and may ship in a polish wave; consider carving them as Phase 6 entry tickets.

See `ACTION_ITEMS.md` for the prioritized action list.
