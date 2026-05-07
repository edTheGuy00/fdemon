# Task 01: Modal Precedence and Sub-Modal Gates

## Goal

Fix the critical modal-precedence leak (REVIEW.md Critical #1) and Settings sub-modal click leak (Major #3) by gating *base-UI region recording* in the renderer when a modal `UiMode` is active. Add cross-cutting integration tests. Also add a single right-click universal coverage test (Minor #19).

## Background

`render::view()` in `crates/fdemon-tui/src/render/mod.rs` registers `MainHeader` (line ~132) and `LogView` (line ~181/193) regions at `z_index = 0` *before* the `match state.ui_mode` block at line ~206. The four per-mode dispatchers (`handler/mouse/{confirm_dialog,new_session,tag_filter,settings}.rs`) call `regions.hit_test(x, y, button)` — which returns the highest-z entry whose rect *contains* (x, y). When a click falls *outside* the modal's z=1 rects but lands on an underlying z=0 region (e.g. clicking `[r]` in the header while `ConfirmDialog` is open), the only hit is the header z=0 region, so `Message::HotReload` is returned. The user sees the modal but a hot reload fires.

Additionally, `settings::handle_press` only gates on `state.settings_view_state.editing == true`. Opening `dart_defines_modal` or `extra_args_modal` does NOT set `editing = true`, so settings clicks under the open sub-modal still route to the underlying tab/row. The helper `state.settings_view_state.has_modal_open()` exists at `crates/fdemon-app/src/handler/settings_dart_defines.rs:21`.

## Approach

**Renderer-level gating** (clean, single source of truth): when `state.ui_mode` is a modal mode, do NOT thread `Some(&mut mouse_ctx)` into `MainHeader` or `LogView`. Pass `None` instead. Base-UI z=0 regions are simply not registered while a modal is up. The per-mode dispatchers continue to use plain `hit_test` — there is nothing to gate against.

This is preferred over a per-dispatcher `hit_test_min_z` filter because:
- One change site (renderer) vs. four (dispatchers).
- Future modes (e.g., a Phase-6 sub-modal) inherit the gate by adding to a single match.
- No new public API on `MouseRegions`.
- Settings mode (z=0 SettingsPanel regions) remains correct without z-index reshuffling.

For the sub-modal leak (Major #3), a small additional gate goes at the top of `settings::handle_press` to short-circuit when a Settings sub-modal is open. (Sub-modals don't change `ui_mode`, so the renderer-level approach can't cover them.)

## Files

**Modify:**
- `crates/fdemon-tui/src/render/mod.rs` — gate base-UI ctx threading by `ui_mode`
- `crates/fdemon-app/src/handler/mouse/settings.rs` — add `has_modal_open()` early return
- `crates/fdemon-app/src/handler/tests.rs` — add `phase5_5_modal_precedence_tests` module

**Read (reference):**
- `crates/fdemon-app/src/state.rs` — `UiMode` variants, `SettingsViewState::has_modal_open` access path
- `crates/fdemon-app/src/handler/settings_dart_defines.rs` — `has_modal_open()` helper
- `crates/fdemon-app/src/handler/mouse/{confirm_dialog,new_session,tag_filter}.rs` — confirm no changes needed
- `crates/fdemon-app/src/mouse_regions.rs` — confirm `hit_test` semantics

## Plan

1. **In `render::view()`**, define a helper that returns whether the current `ui_mode` is a modal mode that should suppress base-UI region recording:

   ```rust
   fn is_modal_ui_mode(mode: &UiMode) -> bool {
       matches!(
           mode,
           UiMode::Startup
               | UiMode::NewSessionDialog
               | UiMode::ConfirmDialog
               | UiMode::Settings
               | UiMode::FlutterVersion
               | UiMode::EmulatorSelector
       )
   }
   ```

   Adapt to the actual `UiMode` enum. `UiMode::Normal` with `tag_filter_visible == true` is also "modal" for click purposes — handle that with an OR clause:
   ```rust
   let in_modal = is_modal_ui_mode(&state.ui_mode) || state.tag_filter_visible;
   ```

   `UiMode::LinkHighlight` is NOT modal — links live atop the log view and the user expects the log view to remain interactive (e.g., scrolling). Keep base-UI regions registered there. Verify by reading existing `link_highlight::handle_press`.

2. **Thread `None` instead of `Some(&mut mouse_ctx)` for MainHeader and LogView when `in_modal`:**

   ```rust
   let header_ctx: Option<&mut MouseCtx<'_>> = if in_modal { None } else { Some(&mut mouse_ctx) };
   widgets::header::render_main_header(areas.header, frame.buffer_mut(), &header, header_ctx);
   ```

   Same pattern for the LogView render calls (both the `selected_mut` and empty-state branches). The `Option<&mut MouseCtx<'_>>` type doesn't `Copy`, so this requires careful borrow choreography — likely re-acquire `mouse_ctx` via `regions.builder()` for the modal block, or split the function. Audit the current borrow shape and adapt.

   **Implementation hint:** rather than juggling two `Option<&mut MouseCtx>` borrows of the same builder, conditionally write `is_modal` once at the top, then in each call site do `if is_modal { None } else { Some(&mut mouse_ctx) }`. Rust's NLL should allow this since the borrow ends before the next call.

3. **Add the sub-modal gate** at the top of `crates/fdemon-app/src/handler/mouse/settings.rs::handle_press`:
   ```rust
   pub(super) fn handle_press(
       state: &mut AppState,
       x: u16,
       y: u16,
       button: MouseButton,
       _mods: KeyModSet,
   ) -> Option<Message> {
       if button == MouseButton::Right { return None; }
       if state.settings_view_state.has_modal_open() {
           return None;
       }
       if state.settings_view_state.editing {
           return None;
       }
       // ... existing hit_test logic
   }
   ```

4. **Add cross-cutting tests** in `crates/fdemon-app/src/handler/tests.rs` under a new `mod phase5_5_modal_precedence_tests`. These tests exercise the *handler-layer* invariant directly. Setup: register a known z=0 region and a known z=1 region, then in each modal mode assert that the dispatcher honors modal precedence.

   **Important:** these tests can NOT exercise the renderer-level gate (which is in `fdemon-tui`, downstream of the test crate). What they can test is:
   - The Settings sub-modal early-return (the dispatcher code path).
   - That when a registry contains ONLY z=1 regions (because the renderer suppressed z=0 in modal mode — simulated by the test fixture), all four dispatchers correctly route z=1-only clicks.
   - That when a registry contains BOTH z=0 and z=1 regions and a click falls on a z=0 rect not overlapping z=1, the dispatchers return the z=0 message (current behavior — the renderer is responsible for prevention, not the dispatcher).

   Tests:

   - `phase5_5_settings_dispatcher_with_dart_defines_modal_open_returns_none` — open dart_defines modal, register a z=0 settings tab region at (10, 4). Assert `settings::handle_press(state, 10, 4, Left, NONE)` returns `None`.
   - `phase5_5_settings_dispatcher_with_extra_args_modal_open_returns_none` — same for extra_args.
   - `phase5_5_renderer_invariant_modal_modes_register_no_main_header_regions` — render via `render::view()` with `ui_mode = ConfirmDialog`, inspect the resulting registry, assert no `Message::HotReload` (header `[r]`) region is present. **This test lives in `crates/fdemon-tui/src/render/tests.rs`, NOT in handler/tests.rs.** Co-locate with similar Phase-5 invariant tests there.
   - `phase5_5_renderer_invariant_normal_mode_with_tag_filter_registers_no_main_header_regions` — same, with `ui_mode = Normal` and `tag_filter_visible = true`. Lives in `render/tests.rs`.
   - `phase5_5_renderer_invariant_link_highlight_keeps_main_header_regions` — sanity check that LinkHighlight is NOT modal. Lives in `render/tests.rs`.
   - **Right-click universal coverage** (Minor #19): `phase5_5_right_click_is_no_op_in_all_ui_modes` — iterate `UiMode::Normal`, `DevTools`, `ConfirmDialog`, `Settings`, `NewSessionDialog`, `LinkHighlight`, `Loading`, `EmulatorSelector`, `SearchInput`, `FlutterVersion`. For each, dispatch via the appropriate per-mode handler with `Right` button and assert `None`. Lives in `handler/tests.rs`.

   **Update Files Modified to also include `crates/fdemon-tui/src/render/tests.rs`** for the renderer-invariant tests (3 of them).

5. **Run quality gates**:
   ```bash
   cargo test --workspace
   cargo clippy --workspace --all-targets -- -D warnings
   cargo fmt --all -- --check
   cargo check --workspace --all-targets
   ```

## Acceptance Criteria

- [ ] `render::view()` does NOT thread `Some(&mut mouse_ctx)` into `MainHeader` or `LogView` when in a modal `UiMode` (or when `tag_filter_visible`).
- [ ] `settings::handle_press` returns `None` early when `state.settings_view_state.has_modal_open()`.
- [ ] No changes required in the other three dispatchers (`confirm_dialog`, `new_session`, `tag_filter`).
- [ ] 2 new sub-modal handler tests pass.
- [ ] 3 new renderer-invariant tests pass (live in `render/tests.rs`).
- [ ] 1 new right-click universal coverage test passes (lives in `handler/tests.rs`).
- [ ] Existing Phase-5 click-precedence test (`phase5_modal_z1_region_wins_over_base_z0_region_at_same_cell`) still passes (no regression).
- [ ] All quality gates pass.

## Notes

- **No write overlap with T05.** This task does NOT modify `widgets/settings_panel/{mod,tests}.rs`. The renderer-level gate is in `render/mod.rs`. T05 owns the settings panel widget; T01 owns the renderer threading.
- **No write overlap with T09.** T09 modifies a stale comment in `render/tests.rs:87-92`. T01 adds NEW tests at the end of `render/tests.rs`. Different lines; merge-safe but coordinate ordering: T01 should land first to add tests, then T09 adjusts the comment block.

   Actually, T09 only edits the *comment* at lines 87-92, not surrounding code. T01 adds new tests below the existing test functions. The two edits do not interfere unless the file's line numbering changes between them. **Document T01 ↔ T09 as parallel-safe with a "no overlapping line ranges" note in the TASKS.md overlap matrix.**

- **Verify the existing `mouse_ctx` borrow shape:** the current `view()` body uses `Some(&mut mouse_ctx)` repeatedly. Switching to a conditional `Option` may run into borrow-checker friction. If pure conditional ergonomics fail, an alternative is to take `&mut mouse_ctx` once into a binding and pass `None` for the suppressed branches. Or factor the modal/non-modal split into a helper.
- The renderer-level gate is the right Phase-5.5 fix. It does NOT prevent base-UI keyboard events from being intercepted (those are routed through `handler/keys.rs` which already has its own modal-aware logic). Mouse and keyboard remain symmetric: base UI is unreachable while a modal is open.
- **Coordinate with T03:** T03 modifies `widgets/log_view/mod.rs` (badge regions). When T01's renderer change passes `None` for the LogView ctx in modal mode, the badge code (which checks `ctx.is_some()` before recording) automatically skips. No T03 changes needed for compatibility — verify by reading T03's plan.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/render/mod.rs` | Added `is_modal_ui_mode()` helper; added `in_modal` flag; MainHeader and LogView now receive `None` ctx when in modal mode or tag_filter_visible |
| `crates/fdemon-app/src/handler/mouse/settings.rs` | Added `has_modal_open()` early-return gate before the editing gate in `handle_press` |
| `crates/fdemon-app/src/handler/tests.rs` | Added `phase5_5_modal_precedence_tests` module with 3 new tests |
| `crates/fdemon-tui/src/render/tests.rs` | Added 3 renderer-invariant tests; updated 2 stale comments from pre-5.5 behavior |

### Notable Decisions/Tradeoffs

1. **Borrow choreography**: Used local bindings `header_ctx` and `log_ctx` of type `Option<&mut MouseCtx<'_>>` with `if in_modal { None } else { Some(&mut mouse_ctx) }`. NLL handles this correctly since each borrow ends before the next call site. No need to juggle two overlapping borrows or restructure the function.

2. **`is_modal_ui_mode` excludes `LinkHighlight`**: Per the task spec, link badges overlay the log view and the user expects both to remain interactive. Verified against the existing `phase5_view_renders_expected_link_highlight_badge_regions` test which still passes.

3. **Stale test comment updates**: `view_header_regions_present_in_settings_mode_because_header_always_renders` and `phase5_sister_functions_record_no_regions_in_stub_state` had comments stating "header regions are still registered" — updated to reflect Phase-5.5 reality. The assertions themselves still hold because Settings panel registers its own regions.

4. **Settings UiMode in `is_modal_ui_mode`**: Settings is included even though it is a full-screen replacement (not a dialog). The reason is that the header IS rendered visually in Settings mode, but we don't want header shortcuts to fire while the Settings panel is up. Including Settings in the modal gate prevents this.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all tests, no regressions)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- New tests verified individually with `cargo test -- phase5_5` (6/6 pass)
- Existing phase5 tests verified with `cargo test -- phase5` (all pass)

### Risks/Limitations

1. **T09 comment edit**: T09 plans to edit comments at `render/tests.rs:87-92`. This task updated comments in a different location (the ConfirmDialog smoke test and Settings probe test). Line numbers shifted due to added Phase-5.5 tests — T09 should verify its target line range against the current file before applying changes.
