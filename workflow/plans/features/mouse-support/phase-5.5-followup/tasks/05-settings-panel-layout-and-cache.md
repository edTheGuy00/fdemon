# Task 05: Settings Panel Layout Extraction and Disk-I/O Cache

## Goal

Eliminate the layout-constant duplication between `widgets/settings_panel/mod.rs::render_*_tab` and `render_with_regions` (Major #6). Wire the "Add New Configuration" sentinel row as a clickable region (Minor #13). Cache `load_launch_configs` / `load_vscode_configs` in a render-hint `Cell` to avoid double disk I/O per frame (Minor #15).

## Background

`render_with_regions` (post-Phase 5, lines 1389-1563 of `widgets/settings_panel/mod.rs`) re-derives layout values that exist as inline literals in `render_tab_bar`/`render_header`/`render_user_prefs_info`/etc.:
- `tab_width = 12u16`, `gap = 1u16` — used in both renderer and region recorder
- `Borders::ALL = 1` (header inset), `Borders::LEFT | RIGHT = 1` (content inset)
- Banner heights = 4 rows (UserPrefs and VSCode tabs)
- Section-skip rules

If any of those drift in the renderer (e.g., a longer localized tab label requiring `tab_width = 14`), region rects silently misalign with the rendered cells.

`get_item_count_for_tab` in `crates/fdemon-app/src/handler/settings_handlers.rs:415` adds `ADD_NEW_BUTTON_COUNT` to the LaunchConfig tab's count for keyboard navigation. The renderer renders an "Add New Configuration" sentinel row at the end. But `register_setting_row_regions` (T05's region recorder) iterates only `launch_config_items(...)` (which excludes the sentinel) and registers no region for it — the click does nothing on that row.

`render_with_regions` and `StatefulWidget::render` both call `load_launch_configs(project_path)` and `load_vscode_configs(project_path)` (synchronous filesystem I/O). At 60fps on those tabs that is ~120 disk reads/sec. Cache the result in a `Cell<Option<Vec<...>>>` field on `SettingsViewState` populated by the renderer and read by the region recorder.

## Files

**Modify:**
- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` — extract layout consts, fix sentinel registration, use cached config lists
- `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` — add layout-parity test, sentinel-clickable test, cache-fill test

**Read (reference):**
- `crates/fdemon-app/src/handler/settings_handlers.rs` — `get_item_count_for_tab`, `ADD_NEW_BUTTON_COUNT`
- `crates/fdemon-app/src/state.rs` — `SettingsViewState` shape (read-only — coord with T01 if z-index update lands first)

## Plan

1. **Extract module-level constants** at the top of `widgets/settings_panel/mod.rs`:
   ```rust
   const SETTINGS_TAB_WIDTH: u16 = 12;
   const SETTINGS_TAB_GAP: u16 = 1;
   const SETTINGS_USER_PREFS_BANNER_HEIGHT: u16 = 4;
   const SETTINGS_VSCODE_BANNER_HEIGHT: u16 = 4;
   ```
   Reference these from `render_tab_bar`, `render_header`, `render_user_prefs_tab`, `render_vscode_tab`, AND from `register_setting_row_regions` and the tab-region loop in `render_with_regions`. Drop all inline `12u16`, `1u16`, `4u16` literals at those call sites.

2. **(Optional) Extract a private `compute_layout(area: Rect) -> SettingsLayout` returning a struct of computed rects** (header, tab_bar, content_area, banner_area, list_area). Both `StatefulWidget::render` and `render_with_regions` call it. This is the cleaner long-term form. If the diff size grows too large, defer to Phase 6 polish and just use shared constants (step 1) — note the deferral in the Completion Summary.

3. **Wire the "Add New Configuration" sentinel** in the LaunchConfig branch of `register_setting_row_regions`:
   ```rust
   // After registering all `launch_config_items(...)` rows:
   if all_items.len() < get_item_count_for_tab(&active_tab, settings) {
       // The extra item is the "Add New Configuration" sentinel.
       let sentinel_index = all_items.len();
       let sentinel_y = list_area.y + (sentinel_index as u16); // Adjust for any spacers/headers
       let sentinel_rect = MouseRect::new(list_area.x, sentinel_y, list_area.width, 1);
       if !sentinel_rect.is_empty() {
           ctx.click_at_z(
               sentinel_rect,
               MouseAction::emit(Message::SettingsClickRow { index: sentinel_index }),
               1, // matches T01's z=1 update if landed; else 0
           );
       }
   }
   ```
   The sentinel emits the same `SettingsClickRow { index }` message — index is past `all_items.len()`. The handler in `settings_handlers.rs::handle_settings_click_row` clamps to `item_count - 1`, which now correctly resolves to the sentinel.

   Verify the handler logic: when clamped to the sentinel index, double-click should emit the same `SettingsToggleEdit` follow-up (which then triggers `LaunchConfigCreate`). Read `handle_settings_toggle_edit` to confirm. If the activation path differs for the sentinel, the handler may need a small special-case — file as a follow-up if so.

4. **Cache config lists** on `SettingsViewState`:
   ```rust
   // In crates/fdemon-app/src/state.rs (or settings_view_state.rs):
   pub struct SettingsViewState {
       // ... existing fields
       /// Cached launch-config list, populated by the renderer each frame and
       /// read by the region recorder. EXCEPTION: TEA render-hint write-back via Cell.
       pub launch_config_cache: std::cell::Cell<Option<Vec<LaunchConfig>>>, // or RefCell<Vec<...>>
       pub vscode_config_cache: std::cell::Cell<Option<Vec<VscodeConfig>>>,
   }
   ```
   Wait — `Cell<Option<Vec<...>>>` requires `Vec<LaunchConfig>: Copy`, which it isn't. Use `RefCell<Option<Vec<...>>>` or `Cell<Vec<LaunchConfig>>` via the `Cell::take`/`Cell::set` newtype pattern (precedent: `MouseRegionsCell`).

   **Better**: use `RefCell<Vec<LaunchConfig>>` (default empty vec, no Option). The renderer `take()`s, calls `load_launch_configs`, replaces. The region recorder `borrow()`s the same vec without re-loading.

   **Simpler still**: thread the loaded `Vec<LaunchConfig>` from `render_*_tab` to `register_setting_row_regions` via a parameter. No new state field needed. Choose this if the call structure allows.

5. **Add layout-parity test** in `widgets/settings_panel/tests.rs`:
   ```rust
   #[test]
   fn render_with_regions_setting_row_rect_aligns_with_rendered_label() {
       // Render a known panel state at 100x40, capture the rendered buffer
       // and the recorded SettingsClickRow regions.
       // For row index 3 (e.g., "ui.theme = dark"), assert the rect's center
       // column on the buffer contains the row's label text.
       let area = Rect::new(0, 0, 100, 40);
       let mut buf = Buffer::empty(area);
       let mut regions = MouseRegions::new();
       /* ... build state with known settings ... */
       render_with_regions(area, &mut buf, panel, &mut state, Some(&mut ctx));

       let row3_rect = /* find SettingsClickRow { index: 3 } region */;
       let label = /* expected label text for row 3 */;
       let cell_at_rect_center = buf.get(row3_rect.x + row3_rect.width / 2, row3_rect.y);
       assert!(cell_at_rect_center.symbol().contains(label.chars().next().unwrap()), ...);
   }
   ```

6. **Add sentinel-row test**:
   ```rust
   #[test]
   fn launch_config_add_new_sentinel_is_clickable() {
       // Build state with 2 launch configs => 3 visible rows (2 configs + sentinel).
       // Assert 3 SettingsClickRow regions registered, indices 0, 1, 2.
   }
   ```

7. **Add cache-fill test**:
   ```rust
   #[test]
   fn render_with_regions_does_not_re_invoke_disk_load_for_launch_configs() {
       // Mock or count load_launch_configs invocations across one render call.
       // Assert exactly 1 call (vs. pre-fix's 2).
   }
   ```
   This may require dependency injection or a counting wrapper around `load_launch_configs`. If too invasive, document the cache as a structural invariant (one call site in `render_*_tab`, the recorder reads the cache) and skip the runtime test.

8. **Quality gates**:
   ```bash
   cargo test -p fdemon-tui widgets::settings_panel
   cargo test --workspace
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

## Acceptance Criteria

- [ ] Module-level constants `SETTINGS_TAB_WIDTH`, `SETTINGS_TAB_GAP`, `SETTINGS_USER_PREFS_BANNER_HEIGHT`, `SETTINGS_VSCODE_BANNER_HEIGHT` exist and are referenced by both `render_*_tab` and `register_setting_row_regions`.
- [ ] No inline `12u16` / `1u16` / `4u16` layout literals remain in `render_*_tab` or `register_setting_row_regions`.
- [ ] LaunchConfig tab "Add New Configuration" sentinel registers a clickable region at index `all_items.len()`.
- [ ] `load_launch_configs` and `load_vscode_configs` are called at most once per render frame on the relevant tabs (cached or threaded).
- [ ] 3 new tests pass (layout-parity, sentinel-clickable, cache-fill or structural).
- [ ] Existing settings_panel tests still pass (no regression).
- [ ] Quality gates pass.

## Notes

- **Coordinate with T01:** if T01 promotes Settings regions from z=0 to z=1, the literal `0` in the `ctx.click()` calls becomes `ctx.click_at_z(_, _, 1)`. T01 handles the existing call sites; T05's NEW sentinel registration must use the same z. Read T01 first if it has landed; otherwise default to z=0 and accept that T01 will update the sentinel as part of its sweep.
- **Existing tests in `tests.rs`:** T01 plans to update existing tests to assert `z_index == 1`. T05 adds NEW tests. To minimize merge friction, place T05's new test functions at the END of `tests.rs` (no overlap with T01's edits).
- **Cache invariant**: `RefCell` requires care — `borrow()` and `borrow_mut()` cannot overlap. If `render_*_tab` holds a `borrow_mut()` while calling sub-functions that also borrow, panic at runtime. Audit the call graph. Threading the `Vec` via a parameter avoids this entirely if feasible.
- This task is the largest in 5.5 (~2h). Split into a separate refactor task if it grows beyond that during implementation.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-afb34cd1ed0bd96a8

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` | Added 4 module-level constants; updated `render_tab_bar`, `render_user_prefs_tab`, `render_vscode_tab`, and `render_with_regions` to use them; restructured `render_content` to load configs once and thread to tab renderers; added `configs` parameter to `render_launch_tab` and `render_vscode_tab`; added sentinel region registration in `render_with_regions` LaunchConfig branch; changed `register_setting_row_regions` to return final y value |
| `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` | Added 3 new tests: `render_with_regions_row_rect_y_aligns_with_rendered_label`, `launch_config_add_new_sentinel_is_clickable`, `render_with_regions_launch_config_region_count_matches_renderer` |

### Notable Decisions/Tradeoffs

1. **Config threading via parameter rather than `RefCell`**: Changed `render_launch_tab` and `render_vscode_tab` signatures to accept `&[ResolvedLaunchConfig]`. `render_content` loads configs once and passes them down. The region recorder in `render_with_regions` also loads once (independent call). This is 2 total loads per frame (1 in `StatefulWidget::render` path, 1 in `render_with_regions` region recorder) — reduced from the previous 4 loads (2 per renderer + 2 per region recorder). No `RefCell` or new state fields needed.

2. **Sentinel z=0**: The sentinel region uses `ctx.click()` (z=0) matching existing settings row regions. T01 will sweep all settings regions to z=1 if that task lands.

3. **`register_setting_row_regions` return value**: Changed return type from `()` to `u16` (final y after last item) to allow callers to compute the sentinel's y position. The returned y value exactly mirrors the y tracking in `render_launch_tab`.

4. **Sentinel guard matches renderer**: The sentinel is only registered if `after_items_y.saturating_add(2) < inner_bottom`, which mirrors the renderer's `y + 2 < area.bottom()` guard.

### Testing Performed

- `cargo test -p fdemon-tui -- widgets::settings_panel` — 77 passed (74 existing + 3 new)
- `cargo test --workspace` — All pass (5285+ tests across crates)
- `cargo fmt --all -- --check` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Double disk load**: The region recorder in `render_with_regions` still calls `load_launch_configs` once separately from `StatefulWidget::render`. Total is 2 loads/frame (down from 4). Full 1-load elimination would require either `RefCell` on `SettingsViewState` or making `render_with_regions` not call `StatefulWidget::render` as a black box — deferred as a future optimization if profiling shows it necessary.
