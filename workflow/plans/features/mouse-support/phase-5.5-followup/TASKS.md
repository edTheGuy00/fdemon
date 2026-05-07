# Phase 5.5: Mouse Support Follow-up — Task Index

## Overview

Phase 5 of mouse-support shipped clickable regions for `NewSessionDialog`, `ConfirmDialog`, `Settings`, the `TagFilter` overlay, and `LinkHighlight` badges. The implementation review (`workflow/reviews/features/mouse-support/phase-5-dialogs-overlays/REVIEW.md`) returned **NEEDS WORK** with 2 critical findings, 5 major findings, and 13 minor findings. Phase 5.5 closes the 2 critical and all 5 major findings (mandatory before merging Phase 5 to `main`), plus 12 of the 13 minor findings.

The two critical defects are:

1. **Modal-precedence leak** — `view()` registers base-UI z=0 regions (header brackets, log view) before the modal `match` block runs. Per-mode dispatchers in `confirm_dialog`, `new_session`, `tag_filter`, and `settings` call `regions.hit_test(...)` without filtering by `z_index >= 1`, so a click that falls outside the modal's z=1 rects but on an underlying z=0 region (e.g. clicking `[r]` in the header while `ConfirmDialog` is shown) returns the base-UI message. The user sees the dialog but a hot reload fires.
2. **FuzzyModal underflow panic** — `for screen_row in 0..(end - start)` underflows `usize` when `scroll_offset > filtered_indices.len()`. Triggered by typing a no-match query while previously scrolled. Crashes the TUI in debug; ~`usize::MAX` iterations in release.

The five major findings cover Settings sub-modal click leaks, wrap-mode link-badge mis-positioning, ConfirmDialog button centering drift vs. `Alignment::Center`, Settings layout-constant duplication between renderer and region recorder, and a hand-rolled tag-filter scroll computation that may diverge from ratatui's `ListState`. The minor findings consolidate into hygiene tasks across the four modal handlers, two widget directories, and the test suite.

**Total Tasks:** 10
**Estimated Hours:** ~12.5 hours

## Prerequisites

- Phase 5 must be merged on `feat/mouse-support`. All current Phase-5 production code is the baseline for these fixes.
- No new external dependencies. No new crate-level Cargo.toml changes.

## Out of Scope (Deferred to Phase 6)

- **Compact NewSessionDialog mouse hole** (review Minor #20). 40-69 wide × 20-21 tall terminals fall back to the vertical-compact `TargetSelector` layout, which does not record device-row regions. This was explicitly deferred from Phase 5 (Task 09 CONCERN). Phase 6 must either implement compact-vertical regions or add a "Mouse not supported at this size" UI hint. Tracked in Phase 6 entry criteria, not in 5.5.

## Task Dependency Graph

```
                ┌──────────────────────────────────────────────────────┐
                │              No internal dependencies                │
                │  (all 10 tasks run in parallel — single wave)        │
                └──────────────────────────────────────────────────────┘

   ┌────┬────┬────┬────┬────┬────┬────┬────┬────┬────┐
   ▼    ▼    ▼    ▼    ▼    ▼    ▼    ▼    ▼    ▼
┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐
│ 01 ││ 02 ││ 03 ││ 04 ││ 05 ││ 06 ││ 07 ││ 08 ││ 09 ││ 10 │
│mod-││fzy-││wrp-││cnf-││set-││tag-││set-││nws-││rt- ││nws-│
│al  ││und ││bdg ││rnd ││lay ││scr ││hyg ││hyg ││com ││spass│
│gate││gd  ││fix ││cnsl││+che││fix ││tsts││    ││cln ││    │
└────┘└────┘└────┘└────┘└────┘└────┘└────┘└────┘└────┘└────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate / Area |
|---|------|--------|------------|------------|--------------|
| 1 | [01-modal-precedence-and-submodal-gates](tasks/01-modal-precedence-and-submodal-gates.md) | Not Started | — | 2.5h | `fdemon-app` |
| 2 | [02-fuzzy-modal-underflow-guard](tasks/02-fuzzy-modal-underflow-guard.md) | Not Started | — | 0.5h | `fdemon-tui` |
| 3 | [03-wrap-mode-link-badge-y-position](tasks/03-wrap-mode-link-badge-y-position.md) | Not Started | — | 1.5h | `fdemon-tui` |
| 4 | [04-confirm-dialog-render-consolidation](tasks/04-confirm-dialog-render-consolidation.md) | Not Started | — | 1.25h | `fdemon-tui`, `fdemon-app` |
| 5 | [05-settings-panel-layout-and-cache](tasks/05-settings-panel-layout-and-cache.md) | Not Started | — | 2.0h | `fdemon-tui` |
| 6 | [06-tag-filter-scroll-and-const-fix](tasks/06-tag-filter-scroll-and-const-fix.md) | Not Started | — | 1.5h | `fdemon-tui`, `fdemon-app` |
| 7 | [07-settings-handlers-hygiene](tasks/07-settings-handlers-hygiene.md) | Not Started | — | 1.0h | `fdemon-app` |
| 8 | [08-new-session-handler-hygiene](tasks/08-new-session-handler-hygiene.md) | Not Started | — | 0.5h | `fdemon-app` |
| 9 | [09-render-tests-stale-comment](tasks/09-render-tests-stale-comment.md) | Not Started | — | 0.25h | `fdemon-tui` |
| 10 | [10-new-session-single-pass-render](tasks/10-new-session-single-pass-render.md) | Not Started | — | 1.0h | `fdemon-tui` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-modal-precedence-and-submodal-gates | `crates/fdemon-tui/src/render/mod.rs`, `crates/fdemon-tui/src/render/tests.rs`, `crates/fdemon-app/src/handler/mouse/settings.rs`, `crates/fdemon-app/src/handler/tests.rs` | `crates/fdemon-app/src/state.rs` (`UiMode` variants), `crates/fdemon-app/src/handler/settings_dart_defines.rs` (`has_modal_open`), `crates/fdemon-app/src/mouse_regions.rs` (`hit_test` semantics) |
| 02-fuzzy-modal-underflow-guard | `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs` | n/a |
| 03-wrap-mode-link-badge-y-position | `crates/fdemon-tui/src/widgets/log_view/mod.rs`, `crates/fdemon-tui/src/widgets/log_view/tests.rs` | `crates/fdemon-app/src/state.rs` (`LogViewState::wrap_mode`, `LinkHighlightState`) |
| 04-confirm-dialog-render-consolidation | `crates/fdemon-tui/src/widgets/confirm_dialog.rs`, `crates/fdemon-app/src/confirm_dialog.rs` | `crates/fdemon-app/src/state.rs` (read-only for dialog state shape) |
| 05-settings-panel-layout-and-cache | `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`, `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` | `crates/fdemon-app/src/handler/settings_handlers.rs` (`get_item_count_for_tab`) |
| 06-tag-filter-scroll-and-const-fix | `crates/fdemon-tui/src/widgets/tag_filter.rs`, `crates/fdemon-app/src/state.rs` (add `Cell<usize>` for ListState offset write-back) | `crates/fdemon-app/src/session/native_tags.rs` |
| 07-settings-handlers-hygiene | `crates/fdemon-app/src/handler/settings_handlers.rs` | n/a |
| 08-new-session-handler-hygiene | `crates/fdemon-app/src/handler/new_session/clicks.rs`, `crates/fdemon-app/src/handler/new_session/mod.rs` | `crates/fdemon-app/src/new_session_dialog/target_selector.rs` (`flat_list`, `DeviceListItem`) |
| 09-render-tests-stale-comment | `crates/fdemon-tui/src/render/tests.rs` | n/a — comment-only edit (lines 87-92) |
| 10-new-session-single-pass-render | `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` (read-only — verify single-pass invariant), `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs` (read-only) |

### Overlap Matrix

Wave 1 (no internal dependencies): all 10 tasks.

| Task Pair | Wave | Shared Write Files | Isolation Strategy |
|-----------|------|--------------------|---------------------|
| 01 + 02 | 1 | None — T01 writes `fdemon-app/handler/`, T02 writes `fdemon-tui/widgets/new_session_dialog/fuzzy_modal.rs` | **Parallel (worktree)** |
| 01 + 03 | 1 | None — T01 writes handlers, T03 writes `widgets/log_view/` | **Parallel (worktree)** |
| 01 + 04 | 1 | None — T01 writes handlers + `handler/tests.rs`, T04 writes `widgets/confirm_dialog.rs` and `fdemon-app/src/confirm_dialog.rs` | **Parallel (worktree)** |
| 01 + 05 | 1 | None — T05 writes `widgets/settings_panel/{mod,tests}.rs` | **Parallel (worktree)** |
| 01 + 06 | 1 | None — T06 writes `widgets/tag_filter.rs` + `fdemon-app/state.rs` | **Parallel (worktree)** |
| 01 + 07 | 1 | None — T07 writes `handler/settings_handlers.rs` | **Parallel (worktree)** |
| 01 + 08 | 1 | None — T08 writes `handler/new_session/{clicks,mod}.rs` | **Parallel (worktree)** |
| 01 + 09 | 1 | **`render/tests.rs`** — T01 adds new tests at end of file; T09 edits comment at lines 87-92. Disjoint line ranges. | **Parallel (worktree)** with note: if merge-conflict on `render/tests.rs`, run sequentially T01 → T09 |
| 01 + 10 | 1 | None — T10 writes `widgets/new_session_dialog/mod.rs` (call sites only) | **Parallel (worktree)** |
| 02 + 03 | 1 | None | **Parallel (worktree)** |
| 02 + 04 | 1 | None | **Parallel (worktree)** |
| 02 + 05 | 1 | None | **Parallel (worktree)** |
| 02 + 06 | 1 | None | **Parallel (worktree)** |
| 02 + 07 | 1 | None | **Parallel (worktree)** |
| 02 + 08 | 1 | None | **Parallel (worktree)** |
| 02 + 09 | 1 | None | **Parallel (worktree)** |
| 02 + 10 | 1 | None — T02 writes `fuzzy_modal.rs`, T10 writes `mod.rs` only | **Parallel (worktree)** |
| 03 + 04 | 1 | None | **Parallel (worktree)** |
| 03 + 05 | 1 | None | **Parallel (worktree)** |
| 03 + 06 | 1 | None | **Parallel (worktree)** |
| 03 + 07 | 1 | None | **Parallel (worktree)** |
| 03 + 08 | 1 | None | **Parallel (worktree)** |
| 03 + 09 | 1 | None | **Parallel (worktree)** |
| 03 + 10 | 1 | None | **Parallel (worktree)** |
| 04 + 05 | 1 | None | **Parallel (worktree)** |
| 04 + 06 | 1 | None — T04 writes `fdemon-app/confirm_dialog.rs`, T06 writes `fdemon-app/state.rs` | **Parallel (worktree)** |
| 04 + 07 | 1 | None | **Parallel (worktree)** |
| 04 + 08 | 1 | None | **Parallel (worktree)** |
| 04 + 09 | 1 | None | **Parallel (worktree)** |
| 04 + 10 | 1 | None | **Parallel (worktree)** |
| 05 + 06 | 1 | None — T05 writes `widgets/settings_panel/`, T06 writes `widgets/tag_filter.rs` | **Parallel (worktree)** |
| 05 + 07 | 1 | None | **Parallel (worktree)** |
| 05 + 08 | 1 | None | **Parallel (worktree)** |
| 05 + 09 | 1 | None | **Parallel (worktree)** |
| 05 + 10 | 1 | None | **Parallel (worktree)** |
| 06 + 07 | 1 | None | **Parallel (worktree)** |
| 06 + 08 | 1 | None | **Parallel (worktree)** |
| 06 + 09 | 1 | None | **Parallel (worktree)** |
| 06 + 10 | 1 | None | **Parallel (worktree)** |
| 07 + 08 | 1 | None — T07 writes `handler/settings_handlers.rs`, T08 writes `handler/new_session/{clicks,mod}.rs` | **Parallel (worktree)** |
| 07 + 09 | 1 | None | **Parallel (worktree)** |
| 07 + 10 | 1 | None | **Parallel (worktree)** |
| 08 + 09 | 1 | None | **Parallel (worktree)** |
| 08 + 10 | 1 | None | **Parallel (worktree)** |
| 09 + 10 | 1 | None | **Parallel (worktree)** |

Notes on overlap analysis:

- **T01 takes the renderer-level approach** (per ACTION_ITEMS.md Option (b)): `render::view()` skips threading `Some(&mut mouse_ctx)` into `MainHeader`/`LogView` when in a modal `UiMode` or when `tag_filter_visible`. This means base-UI z=0 regions are simply not registered while a modal is up, so the existing per-mode dispatchers using `hit_test` are correct without modification. T01 only modifies one dispatcher (`settings.rs`) for the sub-modal gate. Renderer-invariant tests live in `render/tests.rs`; sub-modal handler test + right-click universal test live in `handler/tests.rs`.
- **T02 owns `fuzzy_modal.rs`** for the underflow fix only. T10 explicitly does not modify `fuzzy_modal.rs` — it only refactors call sites in `new_session_dialog/mod.rs`.
- **T04 owns both `widgets/confirm_dialog.rs` and `fdemon-app/src/confirm_dialog.rs`.** The state struct is widened (optional `warning: Option<String>` field) and the widget is consolidated to delegate `Widget::render` to `render_with_regions(_, _, _, None)`.
- **T05 owns the settings panel widget.** Constants extracted at the top of `mod.rs` are referenced from both `render_*_tab` and `render_with_regions`. The disk-I/O cache is a new `Cell<Vec<...>>` on `SettingsViewState`.
- **T06 owns `widgets/tag_filter.rs` and adds a `Cell<usize>` field to `state.rs`** for ratatui's `ListState.offset()` write-back. No other task writes `state.rs` in 5.5.
- **T07 ↔ T01 do not overlap.** T07 modifies `handler/settings_handlers.rs` (production helpers, stub fix, test renames); T01 modifies `handler/mouse/settings.rs` (dispatcher gates). Different files.
- **T08 modifies both `clicks.rs` (header guard) and `mod.rs` (visibility tightening).** No other task writes either.
- **T09 is comment-only — single-line edits in `render/tests.rs`.**
- **T10 writes only `mod.rs`** to remove the duplicate render-pass for `fuzzy_modal_overlay` and `launch_context`. The single-pass invariant is verified by reading `launch_context.rs` and `fuzzy_modal.rs` (no writes).

## Success Criteria

Phase 5.5 is complete when:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] **Critical #1 closed:** Click on header `[r]` while any modal is open returns `None` from the per-mode dispatcher (verified by 4 new integration tests in `handler/tests.rs`, one per modal mode).
- [ ] **Critical #2 closed:** Typing a no-match fuzzy query while `scroll_offset > 0` does not panic (verified by regression test in `fuzzy_modal.rs::tests`).
- [ ] **Major #3 closed:** `settings::handle_press` returns `None` when `dart_defines_modal` or `extra_args_modal` is open (verified by handler test).
- [ ] **Major #4 closed:** Wrap-mode log line with badge at `col_offset > visible_width` produces a click region at the correct wrapped sub-row (verified by `widgets/log_view/tests.rs` test).
- [ ] **Major #5 closed:** ConfirmDialog button click rect alignment matches the rendered button cells exactly across all `(width - total_width)` parities. `Widget::render` delegates to `render_with_regions(_, _, _, None)` — single source of truth.
- [ ] **Major #6 closed:** Settings panel layout constants (`tab_width`, `tab_gap`, banner heights) are module-level `const`s shared between renderer and region recorder. Layout-parity snapshot test asserts `SettingsClickRow` rect center maps to expected row's label.
- [ ] **Major #7 closed:** Tag-filter scroll-offset uses `Cell<usize>` write-back from ratatui's `ListState` rather than re-implementation. Regression test asserts that for every visible row the recorded `abs_index` matches the visually rendered tag.
- [ ] **Minor #8 closed:** `n_offset` in `tag_filter.rs:271` replaced with named `const N_ACTION_OFFSET`.
- [ ] **Minor #9 closed:** `handle_select_device_at` verifies clamped index is `DeviceListItem::Device(_)`; returns `UpdateResult::none()` for headers.
- [ ] **Minor #10 closed:** `handle_settings_save` and `handle_settings_save_and_close` share a private `save_active_tab` helper.
- [ ] **Minor #11 closed:** Settings double-click test names follow `test_<function>_<scenario>_<expected_result>` convention.
- [ ] **Minor #12 closed:** Stale comment in `render/tests.rs:87-92` updated to reflect post-Phase-5 reality.
- [ ] **Minor #13 closed:** "Add New Configuration" sentinel row in LaunchConfig tab registers a clickable region.
- [ ] **Minor #14 closed:** `new_session_dialog/mod.rs` renders `fuzzy_modal_overlay` and `launch_context` once per frame regardless of `MouseCtx` presence.
- [ ] **Minor #15 closed:** Settings panel disk reads (`load_launch_configs`, `load_vscode_configs`) cached in a render-hint `Cell` rather than called twice per frame.
- [ ] **Minor #16 closed:** "All Flutter processes will be terminated." removed from `confirm_dialog.rs` hardcoded text; moved to optional `warning: Option<String>` on `ConfirmDialogState`.
- [ ] **Minor #17 closed:** `handle_settings_cycle_enum_next/_prev` and `handle_settings_increment` no longer call `mark_dirty()` until they are actually implemented (or implement them).
- [ ] **Minor #18 closed:** `handler/new_session/mod.rs:11` tightened from `pub mod clicks` to `pub(crate) mod clicks`.
- [ ] **Minor #19 closed:** Single integration test in `handler/tests.rs` exercises right-click no-op across all 7 `UiMode` variants.
- [ ] **Manual smoke test (macOS):**
  - Run fdemon in a terminal with each modal open in turn, click on the underlying header `[r]` / `[d]` / `[q]` brackets — no base-UI action fires; the modal stays foreground.
  - Open `Settings` → `LaunchConfig` tab → open `dart_defines` modal → click on the underlying tab bar — no tab change.
  - Open NewSessionDialog → fuzzy modal → type a query that filters all results out while previously scrolled — no panic.
  - Toggle wrap mode in log view, ensure a long line with a badge past visible width is clickable on its actual wrapped row.

## Notes

- **Modal-precedence guard placement (Critical #1):** Per-mode dispatcher gate (Option (a) from REVIEW.md) is preferred over renderer-level base-region suppression. Smaller diff, easier to test, and cleanly co-located with each modal's dispatch logic. The renderer-level approach (Option (b)) becomes a Phase 6 follow-up if/when sub-modals proliferate.
- **Why a unified Task 01 for Critical #1 and Major #3:** Both are gate insertions at the top of `settings::handle_press`. The diffs overlap; merging avoids sequential dependency between two tasks. The other three modal dispatchers (`confirm_dialog`, `new_session`, `tag_filter`) only need the z=1 filter (Critical #1).
- **Why Task 06 owns `state.rs`:** Major #7 fix requires a `Cell<usize>` write-back field for ratatui's `ListState.offset()`. This is the only `state.rs` write in 5.5; T04's `confirm_dialog.rs` change is in `crates/fdemon-app/src/confirm_dialog.rs` (a different file).
- **Why Task 04 also touches `fdemon-app/src/confirm_dialog.rs`:** Minor #16 adds `warning: Option<String>` to `ConfirmDialogState`. The widget read of this field (Major #5 consolidation) is in the same task to keep the diff atomic.
- **Why compact NewSessionDialog (Min #20) is deferred:** Reviewing the regression scope, fixing the compact-vertical TargetSelector requires duplicating Task 09's region-recording logic for a different layout path (~50% of Task 09's complexity). Phase 6 polish is the appropriate scope. Phase 5.5's manual smoke test must explicitly check non-compact paths only (a 100+ wide terminal).
- **No new `Message` variants** are added in 5.5. All fixes operate on existing dispatch arms.
- **Test placement:** New cross-cutting tests for Critical #1 and Minor #19 live in `handler/tests.rs` under a new `phase5_5_modal_precedence_tests` module. Per-task tests live alongside the task's production code (in the relevant `tests.rs`).
