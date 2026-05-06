# Phase 5: Dialogs & Overlays — Task Index

## Overview

Phase 5 extends the per-frame mouse region registry from Phases 3–4 to cover every remaining clickable surface in the TUI: the **NewSessionDialog**, the **ConfirmDialog**, the **TagFilter** overlay, the **LinkHighlight** badges, and the **Settings panel**. Concretely, after Phase 5:

1. **NewSessionDialog** — `[1] Connected` / `[2] Bootable` tab headers select the tab; each device row sets the selected device and confirms (`Message::NewSessionDialogSelectDeviceAt { index }`); each launch-context field (`Configuration` / `Mode` / `Flavor` / `Entry Point` / `Dart Defines`) becomes clickable to focus + activate (`Message::NewSessionDialogFocusField { field }`); the `Launch` button reuses `Message::NewSessionDialogLaunch`. Inside the fuzzy modal, every visible result row is clickable (`Message::NewSessionDialogFuzzySelectAt { index }`). The dart-defines modal stays keyboard-only in v1 (deferred — see Notes).
2. **ConfirmDialog** — `[y] Yes` and `[n] No` buttons emit the action stored in `state.confirm_dialog_state.actions[i].1` (typically `ConfirmQuit` / `CancelQuit`).
3. **TagFilter overlay** — every tag row becomes clickable: `Message::TagFilterClickRow { index: usize }` sets `tag_filter_ui.selected_index` *and* toggles visibility in a single click. The footer's `[a] All` and `[n] None` action labels emit the existing `Message::ShowAllNativeTags` / `Message::HideAllNativeTags`.
4. **LinkHighlight** — every shortcut badge (`[1]`, `[a]`, etc.) rendered in `widgets/log_view/mod.rs` becomes clickable, emitting `Message::SelectLink(c)` for the badge's character. Click targets are badge-width only (3 cells: `[`, char, `]`) to keep the hit zone precise.
5. **Settings panel** — `1. PROJECT` / `2. USER` / `3. LAUNCH` / `4. VSCODE` tab headers emit `Message::SettingsGotoTab(i)`. Each setting row emits `Message::SettingsClickRow { index }`; a single click selects (sets `selected_index`); a second click on the same row within 400 ms emits a follow-up `Message::SettingsToggleEdit` via `UpdateResult::message` (mirrors the Phase 4 `ClickLogRow` → `ToggleStackTraceForEntry` chained-message pattern). The Settings dart-defines / extra-args sub-modals are **deferred** to Phase 6 polish.

The dispatcher in `handler/mouse/mod.rs` is reworked: the `tag_filter_visible` short-circuit in `handle_press` is replaced with a route to a new `handler/mouse/tag_filter.rs::handle_press`. New per-mode press handlers are added for `UiMode::ConfirmDialog`, `UiMode::Settings`, `UiMode::Startup` / `UiMode::NewSessionDialog`, and `UiMode::LinkHighlight`. **Modal precedence (z-index) is exercised for the first time** — every region recorded in this phase that lives on a modal/overlay surface uses `z_index = 1` (or `2` for sub-modals layered atop a primary modal). Right-click and Drag/Release remain no-ops.

When Phase 5 is done, every visible UI surface that has a keyboard activator also responds to clicks. The mouse-only walk-through (open `NewSessionDialog` → click `[1] Connected` → click a device → click `Launch` → click `[r]` to reload → click `[d]` for DevTools → click a frame bar → click `[q]` to quit → click `Yes`) succeeds end-to-end.

**Total Tasks:** 11
**Estimated Hours:** ~13.5 hours

## Prerequisites

- Phases 1–4 plus Phase 4.5 must be merged on `feat/mouse-support`. The registry, `MouseRegionGuard`, `MouseCtx` plumbing, sister `render_with_regions` pattern, double-click chained-message pattern (Phase 4 `ClickLogRow`), and the dispatcher's tag-filter gate must already be in place.
- No new external dependencies. `fdemon-app` continues not to depend on `ratatui`; the registry uses `MouseRect` and the TUI converts `ratatui::layout::Rect` at the boundary.

## Task Dependency Graph

```
Wave 1 (parallel — different crates):
┌────────────────────────────────────┐  ┌────────────────────────────────────┐
│ 01 - phase5-messages-and-state     │  │ 02 - tui-region-plumbing-for-      │
│ (message.rs + state.rs +           │  │      dialogs-and-overlays          │
│  update.rs delegate arms +         │  │ (render/mod.rs +                   │
│  handler/settings_handlers.rs      │  │  widgets/{new_session_dialog,      │
│  stub +                            │  │  confirm_dialog,settings_panel,    │
│  tag-filter click-row inline       │  │  tag_filter,log_view}/* sister     │
│  arm stub)                         │  │  render_with_regions fns)          │
└──────────────┬─────────────────────┘  └──────────────┬─────────────────────┘
               │                                       │
       ┌───────┴────────┬────────────────────┐         │
Wave 2:▼                ▼                    ▼         ▼
┌──────────────┐ ┌──────────────────┐ ┌──────────────┐ ┌──────────────────┐
│ 03 - settings│ │ 04 - tag-filter- │ │ 05 - mouse-  │ │ 06 - confirm-    │
│   -click-row-│ │   click-row-     │ │   press-     │ │   dialog-button- │
│   handler-   │ │   handler-body   │ │   dispatchers│ │   regions        │
│   body       │ │ (update.rs       │ │   -multi-    │ │ (widgets/        │
│ (handler/    │ │  arm body)       │ │   mode       │ │  confirm_dialog) │
│  settings_   │ │                  │ │ (handler/    │ │                  │
│  handlers.rs │ │                  │ │  mouse/*.rs) │ │                  │
│  + state)    │ │                  │ │              │ │                  │
└──────────────┘ └──────────────────┘ └──────────────┘ └──────────────────┘

           ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
Wave 2 ►   │ 07 - tag-filter- │ │ 08 - link-       │ │ 09 - new-session │
(continued)│   overlay-       │ │   highlight-     │ │   -dialog-       │
           │   regions        │ │   badge-regions  │ │   regions        │
           │ (widgets/        │ │ (widgets/        │ │ (widgets/        │
           │  tag_filter.rs)  │ │  log_view/mod.rs)│ │  new_session_    │
           │                  │ │                  │ │  dialog/*)       │
           └──────────────────┘ └──────────────────┘ └──────────────────┘
                                                       ┌──────────────────┐
Wave 2 (continued)                                     │ 10 - settings-   │
                                                       │   panel-regions  │
                                                       │ (widgets/        │
                                                       │  settings_panel/ │
                                                       │  mod.rs)         │
                                                       └──────────────────┘
                                                                │
Wave 3:                                                         ▼
┌──────────────────────────────────────────────────────────────────┐
│ 11 - integration-and-snapshot-tests                              │
│ (handler/tests.rs + render/tests.rs + per-dialog snapshot tests  │
│  + click-precedence z-index test + mouse-only walk-through)      │
└──────────────────────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate / Area |
|---|------|--------|------------|------------|--------------|
| 1 | [01-phase5-messages-and-state](tasks/01-phase5-messages-and-state.md) | Not Started | — | 1.5h | `fdemon-app` |
| 2 | [02-tui-region-plumbing-for-dialogs-and-overlays](tasks/02-tui-region-plumbing-for-dialogs-and-overlays.md) | Not Started | — | 1.75h | `fdemon-tui` |
| 3 | [03-settings-click-row-handler-body](tasks/03-settings-click-row-handler-body.md) | Not Started | 1 | 1h | `fdemon-app` |
| 4 | [04-tag-filter-click-row-handler-body](tasks/04-tag-filter-click-row-handler-body.md) | Not Started | 1 | 0.5h | `fdemon-app` |
| 5 | [05-mouse-press-dispatchers-multi-mode](tasks/05-mouse-press-dispatchers-multi-mode.md) | Not Started | 1 | 1.75h | `fdemon-app` |
| 6 | [06-confirm-dialog-button-regions](tasks/06-confirm-dialog-button-regions.md) | Not Started | 1, 2 | 1h | `fdemon-tui` |
| 7 | [07-tag-filter-overlay-regions](tasks/07-tag-filter-overlay-regions.md) | Not Started | 1, 2 | 1h | `fdemon-tui` |
| 8 | [08-link-highlight-badge-regions](tasks/08-link-highlight-badge-regions.md) | Not Started | 1, 2 | 1h | `fdemon-tui` |
| 9 | [09-new-session-dialog-regions](tasks/09-new-session-dialog-regions.md) | Not Started | 1, 2 | 2h | `fdemon-tui` |
| 10 | [10-settings-panel-regions](tasks/10-settings-panel-regions.md) | Not Started | 1, 2 | 1.25h | `fdemon-tui` |
| 11 | [11-integration-and-snapshot-tests](tasks/11-integration-and-snapshot-tests.md) | Not Started | 3, 4, 5, 6, 7, 8, 9, 10 | 1.5h | `fdemon-app`, `fdemon-tui` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-phase5-messages-and-state | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/settings_handlers.rs` | `crates/fdemon-app/src/new_session_dialog/launch_context.rs` (for `LaunchContextField`), `crates/fdemon-app/src/state.rs` (for `LogClickStamp` precedent) |
| 02-tui-region-plumbing-for-dialogs-and-overlays | `crates/fdemon-tui/src/render/mod.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`, `crates/fdemon-tui/src/widgets/confirm_dialog.rs`, `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`, `crates/fdemon-tui/src/widgets/tag_filter.rs`, `crates/fdemon-tui/src/widgets/log_view/mod.rs` | `crates/fdemon-app/src/mouse_regions.rs`, `crates/fdemon-app/src/message.rs` |
| 03-settings-click-row-handler-body | `crates/fdemon-app/src/handler/settings_handlers.rs`, `crates/fdemon-app/src/state.rs` (add `last_settings_click` if not added in Task 01) | `crates/fdemon-app/src/handler/log_view.rs` (template — `handle_click_log_row` double-click logic) |
| 04-tag-filter-click-row-handler-body | `crates/fdemon-app/src/handler/update.rs` | `crates/fdemon-app/src/state.rs` (`tag_filter_ui`), `crates/fdemon-app/src/session/native_tags.rs` (`toggle_tag`, `sorted_tags`) |
| 05-mouse-press-dispatchers-multi-mode | `crates/fdemon-app/src/handler/mouse/mod.rs`, `crates/fdemon-app/src/handler/mouse/settings.rs`, `crates/fdemon-app/src/handler/mouse/new_session.rs`, `crates/fdemon-app/src/handler/mouse/link_highlight.rs`, `crates/fdemon-app/src/handler/mouse/confirm_dialog.rs` (NEW), `crates/fdemon-app/src/handler/mouse/tag_filter.rs` (NEW) | `crates/fdemon-app/src/mouse_regions.rs`, `crates/fdemon-app/src/handler/mouse/normal.rs` (template), `crates/fdemon-app/src/handler/mouse/devtools.rs` (template) |
| 06-confirm-dialog-button-regions | `crates/fdemon-tui/src/widgets/confirm_dialog.rs` | `crates/fdemon-app/src/confirm_dialog.rs` (`ConfirmDialogState::actions`), `crates/fdemon-app/src/message.rs` |
| 07-tag-filter-overlay-regions | `crates/fdemon-tui/src/widgets/tag_filter.rs` | `crates/fdemon-app/src/message.rs` (`TagFilterClickRow`, `ShowAllNativeTags`, `HideAllNativeTags`) |
| 08-link-highlight-badge-regions | `crates/fdemon-tui/src/widgets/log_view/mod.rs` | `crates/fdemon-app/src/message.rs` (`SelectLink`), `crates/fdemon-app/src/session/link_highlight.rs` (badge rect math) |
| 09-new-session-dialog-regions | `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs` | `crates/fdemon-app/src/message.rs` (`NewSessionDialogSwitchTab`, `NewSessionDialogSelectDeviceAt`, `NewSessionDialogFocusField`, `NewSessionDialogLaunch`, `NewSessionDialogFuzzySelectAt`) |
| 10-settings-panel-regions | `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` | `crates/fdemon-app/src/message.rs` (`SettingsGotoTab`, `SettingsClickRow`) |
| 11-integration-and-snapshot-tests | `crates/fdemon-app/src/handler/tests.rs`, `crates/fdemon-tui/src/render/tests.rs` | All Phase-5 production files |

### Overlap Matrix

Wave 1 (no Phase-5 internal predecessors): 01, 02
Wave 2 (depends on 01 and/or 02): 03, 04, 05, 06, 07, 08, 09, 10
Wave 3 (depends on every Wave-2 task): 11

| Task Pair | Wave | Shared Write Files | Isolation Strategy |
|-----------|------|--------------------|---------------------|
| 01 + 02 | Wave 1 | None — disjoint crates (`fdemon-app` vs `fdemon-tui`) | **Parallel (worktree)** |
| 03 + 04 + 05 | Wave 2 (handlers) | None — `settings_handlers.rs`, `update.rs` arm, `mouse/*` are disjoint write sets *(see note on `update.rs` overlap below — dependency-ordered, not parallel-conflicting)* | **Parallel (worktree)** |
| 06 + 07 + 08 + 09 + 10 | Wave 2 (widgets) | None — each task targets a distinct widget directory | **Parallel (worktree)** |
| 03–10 (across handler/widget) | Wave 2 | None — handlers in `fdemon-app`, widgets in `fdemon-tui`; the only common file the two groups read is `message.rs`, which is read-only after Task 01 lands | **Parallel (worktree)** |
| 11 alone | Wave 3 | n/a — single task | **Single task on current branch** |

Notes on overlap analysis:

- **`update.rs` overlap (01 ↔ 04)** is dependency-ordered. Task 01 inserts a stub arm `Message::TagFilterClickRow { index } => { /* stub */ UpdateResult::none() }` so the dispatch arm compiles; Task 04 fills in the body inline. The write to `update.rs` happens twice but in waves: 01 in Wave 1, 04 in Wave 2 — never concurrent. Same applies to the four `NewSessionDialog*` and `Settings*` arms added in Task 01 and not modified again.
- **`settings_handlers.rs` overlap (01 ↔ 03)** is dependency-ordered for the same reason: Task 01 adds a stub `handle_settings_click_row` returning `UpdateResult::none()`; Task 03 fills in the body and double-click chained-message logic.
- **`state.rs` overlap (01 ↔ 03)**: Task 01 adds `last_settings_click: Option<SettingsClickStamp>` field and the struct definition. Task 03 may revise the field's reset semantics on session change but does not redefine the type. If Task 01 fully captures the field shape (see Task 01 acceptance criteria), Task 03 does not edit `state.rs`. The overlap is therefore "Task 01 writes; Task 03 reads."
- **`widgets/log_view/mod.rs` overlap (02 ↔ 08)** is dependency-ordered: Task 02 stubs the link-badge plumbing through the existing `render_with_regions` (no-op when not in `LinkHighlight` mode); Task 08 fills in the badge-rect recording loop alongside the existing badge-rendering code.
- **`widgets/new_session_dialog/mod.rs` overlap (02 ↔ 09)** is dependency-ordered: Task 02 introduces a sister `render_with_regions(...)` function on `NewSessionDialog` that delegates to the existing `Widget::render` impl with no-op region recording; Task 09 fills in the body and threads `MouseCtx` to `tab_bar.rs`, `device_list.rs`, `launch_context.rs`, and `fuzzy_modal.rs`.
- **`widgets/confirm_dialog.rs` overlap (02 ↔ 06)** is the same story: Task 02 adds the sister function as a no-op; Task 06 fills in the Yes/No button rect math.
- **`widgets/settings_panel/mod.rs` overlap (02 ↔ 10)**: same pattern.
- **`widgets/tag_filter.rs` overlap (02 ↔ 07)**: `tag_filter` is currently a free function, not a `Widget` impl, so Task 02 changes the signature to optionally accept a `&mut MouseCtx` (with `Option<&mut MouseCtx>`); Task 07 fills in the body.
- **`render/mod.rs` is written only by Task 02** in Phase 5. The match arms for `UiMode::Startup`/`NewSessionDialog`, `ConfirmDialog`, `Settings`, and the `tag_filter_visible` branch in `Normal` mode all switch from `frame.render_widget(...)` to `widgets::*::render_with_regions(...)`. No follow-up task re-edits `render/mod.rs`.
- **Cross-crate parallel safety (Wave 2 handlers ↔ widgets)**: Tasks 03/04/05 operate inside `fdemon-app`; Tasks 06/07/08/09/10 operate inside `fdemon-tui`. They communicate only via `Message` variants (defined in Task 01) and the registry API (defined in Phases 1–3). No write overlap; full parallelism.

## Success Criteria

Phase 5 is complete when:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes (no regressions; existing baseline grows by ≥ 25 tests across the new handlers, dispatchers, and widget snapshot suites)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] **Messages exist:** `Message::NewSessionDialogSelectDeviceAt { index: usize }`, `Message::NewSessionDialogFocusField { field: LaunchContextField }`, `Message::NewSessionDialogFuzzySelectAt { index: usize }`, `Message::SettingsClickRow { index: usize }`, `Message::TagFilterClickRow { index: usize }`. No new variants beyond these five.
- [ ] **Existing messages reused:** `Message::NewSessionDialogSwitchTab`, `Message::NewSessionDialogLaunch`, `Message::NewSessionDialogFieldActivate`, `Message::ConfirmQuit`, `Message::CancelQuit` (and the action-coupled variants stored on `ConfirmDialogState`), `Message::SettingsGotoTab`, `Message::SettingsToggleEdit`, `Message::ShowAllNativeTags`, `Message::HideAllNativeTags`, `Message::SelectLink(c)` are all reused for clicks; no parallel variants are added.
- [ ] `AppState::last_settings_click: Option<SettingsClickStamp>` exists (where `SettingsClickStamp { index: usize, at: std::time::Instant }`), reset on tab change and on `SettingsClickRow` follow-up emission. (Mirrors `last_log_click`.)
- [ ] **Settings single-click → no edit-mode entry**, only `selected_index` update. **Settings double-click on same row within 400 ms** produces a follow-up `Message::SettingsToggleEdit`. Verified by handler unit test.
- [ ] **Tag filter row click** sets `tag_filter_ui.selected_index` and toggles the tag's visibility in a single message. Verified by handler unit test.
- [ ] **NewSessionDialog tab click** in the `[1] Connected` / `[2] Bootable` rect emits `Message::NewSessionDialogSwitchTab(tab)`. Verified by snapshot test on the registry.
- [ ] **NewSessionDialog device row click** emits `Message::NewSessionDialogSelectDeviceAt { index }` and the handler sets the selected device index. **Launch button click** emits `Message::NewSessionDialogLaunch` (existing behaviour).
- [ ] **NewSessionDialog field click** (`Configuration` / `Mode` / `Flavor` / `Entry Point` / `Dart Defines`) emits `Message::NewSessionDialogFocusField { field }`; the handler sets `launch_context.focused_field` and emits a follow-up `Message::NewSessionDialogFieldActivate` for fields that toggle on Enter (mirroring keyboard `Enter`).
- [ ] **NewSessionDialog fuzzy modal row click** emits `Message::NewSessionDialogFuzzySelectAt { index }`; handler sets `fuzzy_modal.selected_index = index` and emits a follow-up `Message::NewSessionDialogFuzzyConfirm`.
- [ ] **ConfirmDialog Yes/No click** emits the `Message` stored at the corresponding index in `state.confirm_dialog_state.actions[i].1` (typically `ConfirmQuit` for Yes, `CancelQuit` for No). The button rect covers the `[y] Yes` / `[n] No` span only — not the whole modal.
- [ ] **LinkHighlight badge click** (`[1]` / `[a]` / etc.) emits `Message::SelectLink(c)` for the badge's character. Verified by snapshot test that recorded regions match the rendered badge rects 1:1.
- [ ] `handler/mouse/mod.rs::handle_press` lifts the `tag_filter_visible` short-circuit and routes to a new `handler/mouse/tag_filter.rs::handle_press` that hit-tests against the registry.
- [ ] `handler/mouse/{confirm_dialog,settings,new_session,link_highlight,tag_filter}.rs` each export a `pub(super) fn handle_press(state: &mut AppState, x: u16, y: u16, button: MouseButton, mods: KeyModSet) -> Option<Message>`. Right-click and middle-click in these modes return `None` for v1 (except where explicitly noted).
- [ ] **Modal precedence (z-index) is exercised for the first time in Phase 5.** A click-precedence test verifies that when `NewSessionDialog` is open, a click that would otherwise land on a header `[r]` rect (registered by the underlying log view at `z_index = 0`) is intercepted by the dialog's z=1 region first, returning the dialog's message instead.
- [ ] **No widget renders unconditionally** — every region recording site checks the rect has non-zero area before pushing to the builder (consistent with Phases 3–4 invariant).
- [ ] Snapshot tests on the registry contents:
  - `NewSessionDialog` at 100×40 with one connected device: 2 tab regions, 1 device row region, ≥ 4 launch-context field regions, 1 launch button region. All at `z_index = 1`.
  - `ConfirmDialog` at 80×24: 2 button regions (Yes, No) at `z_index = 1`. Whole-dialog rect not registered (clicks outside buttons are no-op).
  - `TagFilter` overlay at 80×24 with 5 discovered tags: 5 tag-row regions plus 2 action-label regions (`[a] All`, `[n] None`) at `z_index = 1`.
  - `LinkHighlight` log view at 80×24 with 3 detected links: 3 badge regions at `z_index = 0`. Each badge region's rect is exactly 3 cells wide (`[`, char, `]`).
  - `Settings` panel at 100×40 on the Project tab with N items: 4 tab-header regions plus N setting-row regions at `z_index = 0` (Settings panel is full-screen — no underlying base regions to compete with).
- [ ] **Manual smoke test on macOS — mouse-only walk-through:**
  - Run fdemon on a project with no recent session → `NewSessionDialog` opens automatically.
  - Click `[2] Bootable` tab → tab switches.
  - Click `[1] Connected` tab → tab switches back.
  - Click a device row → device is selected.
  - Click `Launch` button → Flutter session starts.
  - In Normal mode, click `[r]` → hot reload triggers (Phase 3 behaviour, regression check).
  - Click session tab → switches session (Phase 3 regression check).
  - Press `T` to open tag filter → click a tag row → tag toggles visibility, list re-renders with new state.
  - Press `Esc` to close tag filter, click `[d]` to open DevTools, click `[p] Performance` → DevTools panel switches (Phase 4 regression check).
  - Press `,` to open Settings → click tab `2. USER` → tab switches. Click a row → row selected. Click same row again within 400 ms → enters edit mode.
  - Press `Esc` to leave Settings, click `[q]` → ConfirmDialog opens. Click `Yes` → fdemon quits.

## Notes

- **Why a new `Message::TagFilterClickRow { index }` instead of reusing `TagFilterMoveToIndex` + `TagFilterToggleSelected`.** A single click on a tag row should both navigate to and toggle the tag — keyboard parity would require pressing arrow keys then Space. Splitting into two messages would force every click to allocate a follow-up via `UpdateResult::message`. A single `TagFilterClickRow` arm in `update.rs` does both updates inline (set `tag_filter_ui.selected_index = index`; toggle the tag at that index) — same code path, fewer allocations, identical user-facing semantics.

- **Why `Message::SettingsClickRow { index }` follows the Phase 4 chained-message pattern instead of toggling on first click.** The PLAN.md says "single click selects, double-click activates" — matching the convention in IDEs and the existing log-view double-click semantic. Single-click toggle would surprise users who clicked a row to inspect its description. The chained-message pattern (see Task 03) reuses the `last_log_click` precedent verbatim, just renamed to `last_settings_click`. We accept the small allocation cost (~1 `Box<Message>` per double-click event, which is rare).

- **Why `Message::NewSessionDialogSelectDeviceAt { index }` instead of `NewSessionDialogDeviceUp/Down + Select` chain.** Click is fundamentally absolute. A relative-direction message stream would require the registry to know how many `Up`/`Down` messages to emit, which is brittle when the visible device count or sort order changes between render and click. `SelectDeviceAt { index }` carries the exact target.

- **Why `Message::NewSessionDialogFocusField { field }` instead of `FieldNext`/`FieldPrev` chain.** Same reasoning as devices.

- **Why the dart-defines and extra-args sub-modals inside `Settings` are deferred.** They are sub-modals layered atop the Settings panel (z=2 over z=1). Adding regions for them inflates Task 10 by ~50% with code that primarily duplicates the NewSessionDialog dart-defines modal pattern. Phase 6 polish will pick them up; the v1 walk-through doesn't need them.

- **Why click handlers (`SettingsClickRow`, `TagFilterClickRow`, etc.) live with their existing module families.** `handle_settings_click_row` belongs in `handler/settings_handlers.rs` next to `handle_settings_toggle_edit`. `TagFilterClickRow` is implemented inline in `update.rs` next to `TagFilterMoveUp` / `TagFilterToggleSelected` (which are also inline). This matches existing conventions; introducing a new `handler/tag_filter.rs` module just for one arm would be premature decomposition.

- **Why `handler/mouse/mod.rs::handle_press` loses its `tag_filter_visible` short-circuit.** Phase 5 needs press hit-testing against the tag-row regions registered by the overlay widget. The new flow is: when `tag_filter_visible`, the dispatcher routes press to `handler/mouse/tag_filter.rs::handle_press`, which hit-tests the registry. The keyboard handler at `handler/keys.rs:105-126` is unchanged (it still intercepts all keys when the overlay is visible) — only the mouse path is reworked.

- **Modal precedence (z-index) policy.** The convention established by Phase 5 and committed to in the PLAN.md is:
  - `z_index = 0` — base UI (header, tabs, log view, DevTools panels, Settings panel rows)
  - `z_index = 1` — primary modals (NewSessionDialog, ConfirmDialog, TagFilter, FlutterVersion)
  - `z_index = 2` — sub-modals layered atop a primary modal (NewSessionDialog fuzzy modal, NewSessionDialog dart-defines modal — when those are wired in Task 09 / Phase 6)
  - `z_index = 3+` — reserved
  Phase 5 widgets that record only at z=0 (Settings panel rows, link badges) document why in their task files (no overlay is present in their `UiMode`).

- **LinkHighlight badge rect convention.** Badges render as `[<char>]` — three cells wide, one cell tall. The recorded `MouseRect` is exactly the three-cell span. Clicks elsewhere on the same row (e.g., on the link's display text adjacent to the badge) are *not* clickable in v1 — narrow click target is intentional to prevent accidental link selection during scroll-to-end gestures. Future enhancement: extend the click target to include the link text.

- **ConfirmDialog button rect convention.** The dialog renders `[y] Yes  [n] No` centered on its button row. Each button's clickable rect covers `[<key>] <label>` — i.e., 4 + label-length cells. The buttons array on `ConfirmDialogState::actions` is iterated in render order, and the registry pushes one region per action with the action's `Message` (e.g., `ConfirmQuit`, `CancelQuit`, `ForceHideSettings`, `SettingsSaveAndClose`, depending on which dialog is open). This means the registry doesn't hard-code `ConfirmQuit`/`CancelQuit` — it reads from `state` so that all confirm dialogs (quit, unsaved-settings, etc.) are clickable for free.

- **Dispatcher gate ordering in `handle_press`.** New ordering (after Phase 5 task 05):
  1. If `tag_filter_visible` → route to `tag_filter::handle_press` (was: return None).
  2. Otherwise dispatch by `state.ui_mode`:
     - `Normal` → `normal::handle_press`
     - `DevTools` → `devtools::handle_press`
     - `ConfirmDialog` → `confirm_dialog::handle_press` (NEW)
     - `Settings` → `settings::handle_press` (NEW)
     - `Startup | NewSessionDialog` → `new_session::handle_press` (NEW)
     - `LinkHighlight` → `link_highlight::handle_press` (NEW)
     - `EmulatorSelector | Loading | SearchInput | FlutterVersion` → return `None` (no clickable surfaces in v1)

- **Snapshot tests assert *registry contents*, not rendered pixels.** Phase 3/4 precedent. Tests render the widget, take the registry, then assert `entries.len()`, per-entry rect math, action shape, and `z_index`. Decoupled from terminal-rendering quirks.

- **Right-click reserved.** Phase 3 deferred right-click context menus indefinitely. Phase 5 maintains that — every new `handle_press` returns `None` for `MouseButton::Right`.

- **Drag/Release reserved.** Phase 5 dispatcher remains drag/release-no-op. Future drag-to-select-text inside `Settings` description column is a Phase 7+ idea.

- **Manual smoke test consolidation.** Per-task acceptance criteria call for narrow checks (e.g., "click `[y]` in ConfirmDialog → quits"). The full mouse-only walk-through lives in Task 11's completion summary. Mirrors Phase 4's pattern.
