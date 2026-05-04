# Phase 3: Region Registry + Clickable Header & Tabs — Task Index

## Overview

Phase 3 introduces the per-frame **mouse region registry** that Phase 4 (log view, DevTools panels) and Phase 5 (dialogs, overlays) will piggyback on. Concretely, this phase:

1. Adds a small `mouse_regions` module to `fdemon-app` with a `MouseRect`, a `MouseAction` enum (`Emit(Message)` and `EmitWithCoord(fn(u16,u16) -> Message)`), a `MouseRegions` registry, a `MouseRegionsBuilder`, and a z-index-aware hit-test.
2. Stores the registry on `AppState` as `Cell<MouseRegions>` (the same exception class as the existing `Cell<usize>` render-hint feedback in `docs/CODE_STANDARDS.md` Principle 3).
3. Threads a `MouseCtx<'a>` from `render::view()` into the widgets that need to record clickable rects (header + tabs in this phase).
4. Adds a hit-test pass to `handler/mouse/normal.rs` for `MouseInput::Press { Left | Middle, .. }` that produces the matched region's `Message`.
5. Records bracketed shortcut rects (`[r] [R] [x] [d] [D] [q]`) in `widgets/header.rs` and per-tab rects + middle-click-close + single-session device-pill rects in `widgets/tabs.rs`.
6. Adds `Message::CloseSessionAt(usize)` so middle-click on tab `i` closes session `i`, regardless of the currently selected tab.

When Phase 3 is done, a user can left-click `[r]` to hot reload, click a session tab to switch, middle-click a tab to close it, and click the device pill in single-session mode to open the New Session dialog. Modal precedence (z-index) is implemented but not exercised — Phase 5 dialogs/overlays will be the first consumers of `z_index = 1`.

Scroll routing is unchanged: it remains coordinate-free in `mod.rs::handle_scroll` (Phase 2). Hit-testing is new and *click-only* in Phase 3.

**Total Tasks:** 8
**Estimated Hours:** ~10 hours

## Prerequisites

- Phase 2 must be merged. `handler/mouse/mod.rs` must already contain the per-mode dispatcher with the `Press` arm currently returning `None`. Phase 3 Task 05 rewrites that arm.
- No new external dependencies. `fdemon-app` continues not to depend on `ratatui` — the registry uses a local `MouseRect` type. The TUI side converts from `ratatui::layout::Rect` at the boundary.

## Task Dependency Graph

```
Wave 1 (no Phase-3 internal deps):
┌────────────────────────────────────┐  ┌────────────────────────────────┐
│ 01 - mouse-regions-module          │  │ 02 - add-close-session-at-     │
│ (NEW fdemon-app/mouse_regions.rs)  │  │      message                   │
│                                    │  │ (message.rs + handler          │
│                                    │  │  + dispatcher)                 │
└──────────────┬─────────────────────┘  └──────────────┬─────────────────┘
               │                                       │
Wave 2:        ▼                                       │
┌────────────────────────────────────┐                 │
│ 03 - state-field-and-exports       │                 │
│ (AppState field + lib.rs exports)  │                 │
└──────────────┬─────────────────────┘                 │
               │                                       │
       ┌───────┴────────┐                              │
Wave 3:▼                ▼                              │
┌──────────────┐ ┌──────────────────┐                  │
│ 04 - tui-    │ │ 05 - handle-     │                  │
│   mouse-ctx- │ │   press-normal-  │                  │
│   plumbing   │ │   mode           │                  │
│ (render::view│ │ (mouse/normal.rs │                  │
│  + MouseCtx) │ │  hit-test)       │                  │
└──────┬───────┘ └────────┬─────────┘                  │
       │                  │                            │
       │   ┌──────────────┘                            │
Wave 4:▼   ▼                                           │
┌──────────────┐ ┌──────────────────┐ ◄────────────────┘
│ 06 - header- │ │ 07 - tabs-and-   │
│   bracket-   │ │   device-pill-   │
│   regions    │ │   regions        │
│ (header.rs)  │ │ (tabs.rs)        │
└──────┬───────┘ └────────┬─────────┘
       │                  │
Wave 5:└────────┬─────────┘
                ▼
┌────────────────────────────────────┐
│ 08 - integration-tests-and-        │
│      snapshot                      │
│ (handler/tests.rs)                 │
└────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate / Area |
|---|------|--------|------------|------------|--------------|
| 1 | [01-mouse-regions-module](tasks/01-mouse-regions-module.md) | Not Started | — | 2h | `fdemon-app` |
| 2 | [02-add-close-session-at-message](tasks/02-add-close-session-at-message.md) | Not Started | — | 1h | `fdemon-app` |
| 3 | [03-state-field-and-exports](tasks/03-state-field-and-exports.md) | Not Started | 1 | 0.5h | `fdemon-app` |
| 4 | [04-tui-mouse-ctx-plumbing](tasks/04-tui-mouse-ctx-plumbing.md) | Not Started | 3 | 1.5h | `fdemon-tui` |
| 5 | [05-handle-press-normal-mode](tasks/05-handle-press-normal-mode.md) | Not Started | 3 | 1h | `fdemon-app` |
| 6 | [06-header-bracket-regions](tasks/06-header-bracket-regions.md) | Not Started | 4 | 1.5h | `fdemon-tui` |
| 7 | [07-tabs-and-device-pill-regions](tasks/07-tabs-and-device-pill-regions.md) | Not Started | 2, 4 | 1.5h | `fdemon-tui` |
| 8 | [08-integration-tests-and-snapshot](tasks/08-integration-tests-and-snapshot.md) | Not Started | 5, 6, 7 | 1h | `fdemon-app`, `fdemon-tui` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-mouse-regions-module | `crates/fdemon-app/src/mouse_regions.rs` (NEW), `crates/fdemon-app/src/lib.rs` (declare `pub(crate) mod mouse_regions;`) | `crates/fdemon-app/src/message.rs` (for `Message` reference type), `crates/fdemon-app/src/input_mouse.rs` (`MouseButton`) |
| 02-add-close-session-at-message | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/session_lifecycle.rs`, `crates/fdemon-app/src/handler/update.rs` | `crates/fdemon-app/src/session_manager.rs` (`remove_session`, `select_by_index`) |
| 03-state-field-and-exports | `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/lib.rs` | `crates/fdemon-app/src/mouse_regions.rs` (Task 01) |
| 04-tui-mouse-ctx-plumbing | `crates/fdemon-tui/src/render/mod.rs`, `crates/fdemon-tui/src/widgets/mod.rs` | `crates/fdemon-app/src/mouse_regions.rs` (Task 01), `crates/fdemon-app/src/state.rs` (Task 03 — `mouse_regions` field) |
| 05-handle-press-normal-mode | `crates/fdemon-app/src/handler/mouse/normal.rs` | `crates/fdemon-app/src/mouse_regions.rs` (Task 01), `crates/fdemon-app/src/state.rs` (Task 03 — `mouse_regions` field), `crates/fdemon-app/src/session_manager.rs` (`any_session_busy`) |
| 06-header-bracket-regions | `crates/fdemon-tui/src/widgets/header.rs` | `crates/fdemon-app/src/mouse_regions.rs` (Task 01), `crates/fdemon-tui/src/render/mod.rs` (Task 04 — `MouseCtx`), `crates/fdemon-app/src/message.rs` (`HotReload`, `HotRestart`, `CloseCurrentSession`, `EnterDevToolsMode`, `ToggleDap`, `RequestQuit`) |
| 07-tabs-and-device-pill-regions | `crates/fdemon-tui/src/widgets/tabs.rs` | `crates/fdemon-app/src/mouse_regions.rs` (Task 01), `crates/fdemon-tui/src/render/mod.rs` (Task 04 — `MouseCtx`), `crates/fdemon-app/src/message.rs` (`SelectSessionByIndex`, `CloseSessionAt` from Task 02, `OpenNewSessionDialog`) |
| 08-integration-tests-and-snapshot | `crates/fdemon-app/src/handler/tests.rs`, `crates/fdemon-tui/src/render/tests.rs` | `crates/fdemon-app/src/mouse_regions.rs`, `crates/fdemon-app/src/state.rs`, `crates/fdemon-tui/src/widgets/header.rs`, `crates/fdemon-tui/src/widgets/tabs.rs` |

### Overlap Matrix

Wave 1 (no Phase-3 internal predecessors): 01, 02
Wave 2 (depends on 01): 03
Wave 3 (depends on 03): 04, 05
Wave 4 (depends on 04): 06, 07 (07 also depends on 02)
Wave 5 (depends on 05, 06, 07): 08

| Task Pair | Wave | Shared Write Files | Isolation Strategy |
|-----------|------|--------------------|---------------------|
| 01 + 02 | Wave 1 | None | **Parallel (worktree)** |
| 03 alone | Wave 2 | n/a | **Single task on current branch** |
| 04 + 05 | Wave 3 | None (different crates) | **Parallel (worktree)** |
| 06 + 07 | Wave 4 | None (`header.rs` vs `tabs.rs`) | **Parallel (worktree)** |
| 08 alone | Wave 5 | n/a | **Single task on current branch** |

Notes on overlap analysis:

- **`lib.rs` overlap (01 ↔ 03)** is dependency-ordered, not parallel-wave overlap — Task 01 finishes (adding `pub(crate) mod mouse_regions;`) before Task 03 starts (adding `pub use mouse_regions::{...}`). No conflict.
- **`render/mod.rs` overlap (04 ↔ 08)** is dependency-ordered. Task 04 writes the threading scaffold; Task 08 only adds a `tests.rs` next to it. The two task files do not collide.
- **Tests inside widget modules**: Task 06 writes header tests in the same file as the production code; Task 07 writes tab tests in the same file. Task 08 keeps cross-cutting integration tests in `handler/tests.rs` and adds a render-level snapshot in `render/tests.rs`. No overlap with 06/07 because 08 does not edit `header.rs` or `tabs.rs`.

## Success Criteria

Phase 3 is complete when:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `crates/fdemon-app/src/mouse_regions.rs` exists with `MouseRect`, `MouseAction { Emit, EmitWithCoord }`, `MouseRegions`, `MouseRegionsBuilder`, and a z-index-aware `hit_test(x, y, button) -> Option<&MouseRegionEntry>` helper
- [ ] `AppState::mouse_regions: Cell<MouseRegions>` exists, default-initialized empty, with the `// EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md Principle 3` doc-comment style
- [ ] `Message::CloseSessionAt(usize)` exists and is dispatched in `handler/update.rs` to a new `session_lifecycle::handle_close_session_at(state, idx)` that mirrors `handle_close_current_session` but operates on an arbitrary session index
- [ ] `render::view()` takes the registry out at frame start with `Cell::take`, hands a `MouseRegionsBuilder` to widgets via `MouseCtx`, and puts the populated registry back via `Cell::set` at frame end
- [ ] `MainHeader::render` records six `MouseAction::Emit(...)` regions in single-session and multi-session modes (one per bracketed shortcut), each covering only the bracket+letter cells (not the trailing label text)
- [ ] `SessionTabs::render` records one region per visible tab (left-click → `SelectSessionByIndex(i)`, middle-click → `CloseSessionAt(i)`); single-session mode also records the device pill rect (left-click → `OpenNewSessionDialog`)
- [ ] `handle_mouse` in `UiMode::Normal` returns the registry-matched `Message` for `MouseInput::Press { button: Left, .. }` and `MouseInput::Press { button: Middle, .. }`. Right-click and Release/Drag remain `None`.
- [ ] Per-mode unit tests cover, at minimum:
  - Left-click on a recorded `[r]` rect → `Some(Message::HotReload)` when not busy; `None` when `state.session_manager.any_session_busy()` is true
  - Left-click on a recorded `[q]` rect → `Some(Message::RequestQuit)` (no busy gate)
  - Left-click on tab rect index 2 → `Some(Message::SelectSessionByIndex(2))`
  - Middle-click on tab rect index 2 → `Some(Message::CloseSessionAt(2))`
  - Left-click on the single-session device pill → `Some(Message::OpenNewSessionDialog)`
  - Left-click outside any registered region → `None`
  - Right-click anywhere → `None` (Phase 5 may revisit; v1 does not bind right-click)
  - z-index precedence: when a `z=1` region overlaps a `z=0` region at the same cell, the `z=1` region wins
- [ ] Snapshot tests on the registry contents:
  - Header at 80×24, single session: 6 bracketed-shortcut regions, in left-to-right order matching `r R x d D q`
  - Header at 120×24, three sessions: 6 bracketed-shortcut regions on the title row + 3 tab regions on the tabs row
  - Tabs at 80×1, nine sessions: 9 tab regions
- [ ] `Message::CloseSessionAt(8)` followed by render → eight sessions remain and the previously-selected session (if not at index 8) is unchanged
- [ ] No new `Message` variants beyond `Message::CloseSessionAt(usize)`
- [ ] No widget renders unconditionally — every region recording site checks that the rect has non-zero area before pushing to the builder
- [ ] `handler/mouse/normal.rs` does not call `state.mouse_regions.take()` — it borrows the registry via a `Cell::take` + put-back pair (or via a non-mutating accessor) so subsequent renders still see a populated registry until the next frame replaces it
- [ ] Manual smoke test on macOS:
  - Start a Flutter session → click `[r]` on the header → hot reload runs
  - Start a Flutter session → click `[r]` while a reload is in flight → no second reload (busy gate respected, no log spam)
  - Start three sessions → click tab 2 → that session is selected → middle-click tab 1 → tab 1 closes; tab 2 (now index 1) remains selected
  - Single-session header → click the right-side device pill → New Session dialog opens
  - Open the Settings panel (`,`) → click anywhere on the header → no action (header regions are not active in `Settings` mode; click is a no-op)

## Notes

- **Why a local `MouseRect`.** `fdemon-app` does not depend on `ratatui` (architectural invariant — only `fdemon-tui` does). The registry lives in `fdemon-app`, so it can't carry a `ratatui::layout::Rect`. We define `pub struct MouseRect { x: u16, y: u16, width: u16, height: u16 }` in `mouse_regions.rs` and convert from `ratatui::layout::Rect` at the TUI boundary. The conversion is trivial and a one-line `impl From<ratatui::layout::Rect> for MouseRect` (defined inside `fdemon-tui`, not `fdemon-app`, to avoid a reverse dependency) keeps call sites tidy.

- **Why `MouseAction` includes `EmitWithCoord` already.** Phase 3 only records `Emit(Message)` regions, but Phase 4 needs `EmitWithCoord(fn(u16, u16) -> Message)` for log-row click → `FocusLogEntryAtRow { row }`, frame-bar click → `SelectFrameAtX { x }`, and network-row click → `SelectNetworkRowAt { row }`. Adding both variants now avoids a Phase 4 refactor; an unused enum variant adds no runtime cost.

- **Why `Cell<MouseRegions>` with put-back rather than `RefCell`.** `Cell::take` returns the contained value by ownership and leaves a `Default::default()` placeholder. The render code takes the registry, populates a fresh builder, then `Cell::set`s the populated registry back. The `handle_mouse` path performs the same dance (take, hit-test, put back). This avoids a runtime borrow-checker (`RefCell::borrow`) and the panic risk it brings, at the cost of a one-frame race window if a hit-test happens between two `Cell::take`s — which it cannot, because both renders and message handling run on the same thread in the TEA loop. Documented inline at every site with the same `// EXCEPTION:` comment style as `Cell<usize>` per `docs/CODE_STANDARDS.md` Principle 3.

- **Why the busy gate lives in `handle_mouse_normal`, not in widget rendering.** Busy state can change between the moment the registry is populated and the moment a click arrives. Gating at registration time would lock in stale state. Gating at click time consults the live `state.session_manager.any_session_busy()`. This mirrors the keyboard handler at `handler/keys.rs:167-173`.

- **Why a new `Message::CloseSessionAt(usize)` instead of reusing `CloseCurrentSession`.** `handle_close_current_session` only operates on the *currently selected* session. Middle-clicking tab 1 while tab 2 is selected must close tab 1 — a different session. The cleanest fix is a new variant that takes an explicit index. The new handler shares the cmd-sender + remove logic with the existing one (refactor a private helper out of `handle_close_current_session`).

- **Why `MouseCtx` is a thread-through parameter, not a `&AppState` accessor inside widgets.** Widgets in `fdemon-tui` are constructed with narrow data references (`&Session`, `&IconSet`) — they generally don't have `&AppState`. Threading `&mut MouseCtx` through the widget constructor keeps render-time side effects explicit and matches PLAN.md's recommendation. The `MouseCtx` struct lives in `fdemon-tui` (it borrows the `MouseRegionsBuilder` from `fdemon-app`), so adding a region is a one-line `ctx.click(rect.into(), Message::HotReload)` call inside the existing render code paths.

- **No coordinate gating for clicks.** Phase 3 click handling is registry-driven: a click coordinate matches at most one region (after z-index ordering). If it matches none, the click is silently dropped. There is no per-`UiMode` whitelist of clickable areas — that emerges naturally from which widgets registered regions during the most recent render of that mode. (E.g., Settings mode does not render the header, so the header regions are not in the registry, so header clicks in Settings are silently dropped.)

- **Right-click reserved.** The plan defers right-click context menus indefinitely. Phase 3 maps right-click to no message in `handle_mouse_normal` to make the intent explicit and to give a target if a future phase adds it.

- **`render::view()` clears the registry at the *start*, not the *end*.** This is an important sequencing detail: if we cleared at the end, a stray click that arrived between two renders could hit-test against last-frame regions, but the layout might have changed (e.g., a session closed, a panel switched). Clearing at the start ensures the registry always reflects the most recently rendered geometry. The trade-off is one frame of "no regions" on the very first render before any widget runs — this is fine because no click can arrive before any render has happened.

- **Snapshot test stability.** Header text (`[r] Run`, etc.) is the source of truth for region recording — the registration loop indexes off the same Span literals. Snapshot tests on the registry catch the drift case where someone changes `[r] Run` to `[r] Reload` without updating the rect math.
