# Phase 2: Scroll Wheel — Task Index

## Overview

Phase 2 replaces the no-op `handle_mouse` shell with concrete per-mode scroll routing. Wheel events flow through `MouseInput::Scroll` and dispatch to the existing per-mode scroll/nav `Message` variants based on `state.ui_mode` and the relevant sub-state (DevTools panel, Settings modal, NewSessionDialog pane, tag-filter overlay, etc.). No hit-testing is introduced — scroll is a global "scroll the focused thing" gesture by current `UiMode`. Shift+wheel triggers page scroll where the existing keymap supports a page step (Normal, LinkHighlight, DevTools/Network).

To set up Phase 3+ (hit-testing per mode) and to give Phase 2 genuine parallelism, Task 01 pre-splits `handler/mouse.rs` into a `handler/mouse/` directory mirroring the existing `handler/devtools/` pattern. Tasks 02–06 then fill in per-mode scroll handlers in their own submodule files and can run in parallel worktrees.

When Phase 2 is done, a user can drop into fdemon and scroll through logs, settings, DevTools panels (Inspector/Network), the Flutter Version panel, link-highlight badges, and every NewSessionDialog/Settings sub-modal with the wheel — and Shift+wheel does page-scroll where applicable.

**Total Tasks:** 7
**Estimated Hours:** ~7.5 hours

## Prerequisites

Phase 1.5 follow-up should land before Phase 2 begins:

- **Phase 1.5 Task 01** (`MouseInput::Click` → `Press` rename) — every Phase 2 task assumes the variant is named `Press`. If Phase 2 lands first, all task references to `MouseInput::Press` must be read as `Click`.
- **Phase 1.5 Task 02** (`assertions_on_constants` clippy fix) — Phase 2 success criteria require `cargo clippy --workspace --all-targets -- -D warnings` to pass; the existing failure in `input_mouse.rs:182-184` blocks that gate.

If Phase 1.5 cannot land first, both items above must be folded into Phase 2 Task 01 as a prelude.

## Task Dependency Graph

```
                        ┌────────────────────────────────────┐
                        │ 01 - mouse-handler-restructure +   │
                        │      scroll-dispatcher-skeleton    │
                        │ (handler/mouse/ directory + stubs) │
                        └──────────────┬─────────────────────┘
                                       │
       ┌─────────────────┬─────────────┼─────────────────┬─────────────────┐
       ▼                 ▼             ▼                 ▼                 ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐ ┌──────────────────┐
│ 02 - normal- │ │ 03 - devtools│ │ 04 - settings│ │ 05 - new-session-│ │ 06 - simple-     │
│      scroll  │ │      scroll  │ │      scroll  │ │      dialog-     │ │      modes-      │
│ (normal.rs)  │ │ (devtools.rs)│ │ (settings.rs)│ │      scroll      │ │      scroll      │
│              │ │              │ │              │ │ (new_session.rs) │ │ (link_highlight  │
│              │ │              │ │              │ │                  │ │  + flutter_      │
│              │ │              │ │              │ │                  │ │  version)        │
└──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └────────┬─────────┘ └────────┬─────────┘
       │                │                │                  │                    │
       └────────────────┴────────────────┴──────────────────┴────────────────────┘
                                          │
                                          ▼
                        ┌────────────────────────────────────┐
                        │ 07 - update-integration-tests      │
                        │ (handler/tests.rs)                 │
                        └────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate / Area |
|---|------|--------|------------|------------|--------------|
| 1 | [01-mouse-handler-restructure](tasks/01-mouse-handler-restructure.md) | Done | — | 1.5h | `fdemon-app` |
| 2 | [02-normal-mode-scroll](tasks/02-normal-mode-scroll.md) | Done | 1 | 1h | `fdemon-app` |
| 3 | [03-devtools-mode-scroll](tasks/03-devtools-mode-scroll.md) | Done | 1 | 1.5h | `fdemon-app` |
| 4 | [04-settings-mode-scroll](tasks/04-settings-mode-scroll.md) | Done | 1 | 1h | `fdemon-app` |
| 5 | [05-new-session-dialog-scroll](tasks/05-new-session-dialog-scroll.md) | Done | 1 | 1h | `fdemon-app` |
| 6 | [06-simple-modes-scroll](tasks/06-simple-modes-scroll.md) | Done | 1 | 0.5h | `fdemon-app` |
| 7 | [07-update-integration-tests](tasks/07-update-integration-tests.md) | Done | 2, 3, 4, 5, 6 | 1h | `fdemon-app` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-mouse-handler-restructure | `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/handler/mouse.rs` (DELETE — content moves), `crates/fdemon-app/src/handler/mouse/mod.rs` (NEW), `crates/fdemon-app/src/handler/mouse/normal.rs` (NEW, stub), `crates/fdemon-app/src/handler/mouse/devtools.rs` (NEW, stub), `crates/fdemon-app/src/handler/mouse/settings.rs` (NEW, stub), `crates/fdemon-app/src/handler/mouse/new_session.rs` (NEW, stub), `crates/fdemon-app/src/handler/mouse/link_highlight.rs` (NEW, stub), `crates/fdemon-app/src/handler/mouse/flutter_version.rs` (NEW, stub), `crates/fdemon-app/src/input_mouse.rs` (add `KeyModSet::is_shift_only`) | `crates/fdemon-app/src/state.rs` (UiMode), `crates/fdemon-app/src/handler/keys.rs` (dispatch pattern reference) |
| 02-normal-mode-scroll | `crates/fdemon-app/src/handler/mouse/normal.rs` | `crates/fdemon-app/src/state.rs` (`tag_filter_visible`), `crates/fdemon-app/src/message.rs` (`ScrollUp`, `ScrollDown`, `PageUp`, `PageDown`, `TagFilterMoveUp`, `TagFilterMoveDown`), `crates/fdemon-app/src/input_mouse.rs` (`is_shift_only`) |
| 03-devtools-mode-scroll | `crates/fdemon-app/src/handler/mouse/devtools.rs` | `crates/fdemon-app/src/state.rs` (`DevToolsPanel`, `devtools_view_state.active_panel`), `crates/fdemon-app/src/session/network.rs` (`filter_input_active`), `crates/fdemon-app/src/message.rs` (`DevToolsInspectorNavigate`, `NetworkNavigate`, `InspectorNav`, `NetworkNav`) |
| 04-settings-mode-scroll | `crates/fdemon-app/src/handler/mouse/settings.rs` | `crates/fdemon-app/src/state.rs` (`settings_view_state`), `crates/fdemon-app/src/message.rs` (`SettingsPrevItem`, `SettingsNextItem`, `SettingsDartDefinesUp/Down`, `SettingsExtraArgsUp/Down`), `crates/fdemon-app/src/new_session_dialog.rs` (`DartDefinesPane`) |
| 05-new-session-dialog-scroll | `crates/fdemon-app/src/handler/mouse/new_session.rs` | `crates/fdemon-app/src/new_session_dialog.rs` (`DialogPane`, `DartDefinesPane`, modal-open helpers), `crates/fdemon-app/src/message.rs` (`NewSessionDialogFuzzyUp/Down`, `NewSessionDialogDartDefinesUp/Down`, `NewSessionDialogDeviceUp/Down`, `NewSessionDialogFieldPrev/Next`) |
| 06-simple-modes-scroll | `crates/fdemon-app/src/handler/mouse/link_highlight.rs`, `crates/fdemon-app/src/handler/mouse/flutter_version.rs` | `crates/fdemon-app/src/message.rs` (`ScrollUp`, `ScrollDown`, `PageUp`, `PageDown`, `FlutterVersionUp`, `FlutterVersionDown`), `crates/fdemon-app/src/input_mouse.rs` (`is_shift_only`) |
| 07-update-integration-tests | `crates/fdemon-app/src/handler/tests.rs` | `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/input_mouse.rs`, `crates/fdemon-app/src/message.rs` |

### Overlap Matrix

Wave 1 (no dependencies on phase-internal predecessors): 01
Wave 2 (depends on 01): 02, 03, 04, 05, 06
Wave 3 (depends on all of wave 2): 07

| Task Pair | Wave | Shared Write Files | Isolation Strategy |
|-----------|------|--------------------|---------------------|
| 01 alone | Wave 1 | n/a | **Single task on current branch** |
| 02 + 03 | Wave 2 | None | **Parallel (worktree)** |
| 02 + 04 | Wave 2 | None | **Parallel (worktree)** |
| 02 + 05 | Wave 2 | None | **Parallel (worktree)** |
| 02 + 06 | Wave 2 | None | **Parallel (worktree)** |
| 03 + 04 | Wave 2 | None | **Parallel (worktree)** |
| 03 + 05 | Wave 2 | None | **Parallel (worktree)** |
| 03 + 06 | Wave 2 | None | **Parallel (worktree)** |
| 04 + 05 | Wave 2 | None | **Parallel (worktree)** |
| 04 + 06 | Wave 2 | None | **Parallel (worktree)** |
| 05 + 06 | Wave 2 | None | **Parallel (worktree)** |
| 07 alone | Wave 3 | n/a | **Single task on current branch** |

All wave-2 task pairs have zero shared write files — Wave 2 is fully parallelizable across five worktrees. Task 01 establishes the `mod.rs` dispatcher with placeholder `None` arms for every mode, so Wave 2 tasks only need to write their own submodule file.

## Success Criteria

Phase 2 is complete when:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `crates/fdemon-app/src/handler/mouse.rs` no longer exists; `crates/fdemon-app/src/handler/mouse/` is a directory module
- [ ] `KeyModSet::is_shift_only()` exists in `input_mouse.rs` and is used by ≥3 modes (Normal, LinkHighlight, DevTools/Network)
- [ ] Per-mode scroll unit tests cover, at minimum:
  - `Normal` (tag-filter off) → `ScrollUp`/`ScrollDown`; Shift → `PageUp`/`PageDown`
  - `Normal` (tag-filter on) → `TagFilterMoveUp`/`TagFilterMoveDown`
  - `LinkHighlight` → `ScrollUp`/`ScrollDown`; Shift → `PageUp`/`PageDown`
  - `DevTools / Inspector` → `DevToolsInspectorNavigate(Up|Down)`
  - `DevTools / Performance` → `None` (no-op by design)
  - `DevTools / Network` (filter inactive) → `NetworkNavigate(Up|Down)`; Shift → `NetworkNavigate(PageUp|PageDown)`
  - `DevTools / Network` (filter active) → `None`
  - `Settings` (no modal, not editing) → `SettingsPrevItem`/`SettingsNextItem`
  - `Settings` (dart-defines List pane) → `SettingsDartDefinesUp/Down`
  - `Settings` (dart-defines Edit pane) → `None`
  - `Settings` (extra-args modal) → `SettingsExtraArgsUp/Down`
  - `Settings` (editing) → `None`
  - `FlutterVersion` → `FlutterVersionUp`/`FlutterVersionDown`
  - `Startup`/`NewSessionDialog` (TargetSelector) → `NewSessionDialogDeviceUp/Down`
  - `Startup`/`NewSessionDialog` (LaunchContext) → `NewSessionDialogFieldPrev/Next`
  - `Startup`/`NewSessionDialog` (fuzzy modal) → `NewSessionDialogFuzzyUp/Down`
  - `Startup`/`NewSessionDialog` (dart-defines modal) → `NewSessionDialogDartDefinesUp/Down`
  - `SearchInput`, `ConfirmDialog`, `EmulatorSelector`, `Loading` → `None`
- [ ] Integration tests in `handler/tests.rs` drive at least 12 distinct `(UiMode, ScrollDir, KeyModSet)` triples through `update(state, Message::Mouse(...))` and assert the resulting `UpdateResult::message`
- [ ] No `Click`-mode (button press), drag, or release routing is wired up — those are explicitly Phase 3+ work and remain `None` for every mode
- [ ] No new `Message` variants are added to `message.rs`; Phase 2 reuses every existing scroll/nav variant
- [ ] No coordinate gating: scroll is global per `UiMode`, regardless of `(x, y)` (the plan defers coordinate-aware scroll to Phase 3+)
- [ ] Manual smoke test on macOS:
  - Run a Flutter session → wheel scrolls log view
  - Open Settings (`,`) → wheel moves item selection
  - Open NewSessionDialog (`+`) → wheel moves device selection
  - Enter DevTools (`d`) → switch to Network panel (`n`) → wheel moves request selection; Shift+wheel page-scrolls
  - Open tag-filter overlay (`T`) → wheel moves tag selection
  - Open Flutter Version panel (`V`) → wheel moves version selection

## Notes

- **Why pre-split `handler/mouse.rs` into a directory.** The `handler/devtools/` directory is the established pattern for splitting a per-area handler when growth is expected. Phase 3 (region registry + clickable header/tabs), Phase 4 (log view + DevTools panel-internal clicks), and Phase 5 (dialogs/overlays) all add per-mode hit-testing logic — likely 100-300 lines per mode in the worst case. Splitting now avoids a churny re-split later, mirrors a project pattern, and gives Phase 2 itself five-way parallelism in Wave 2.
- **`KeyModSet::is_shift_only()` rationale.** Three modes (Normal, LinkHighlight, DevTools/Network) need to detect "Shift held but not Ctrl/Alt" to enable Shift+wheel page-scroll. Inlining `mods.shift && !mods.ctrl && !mods.alt` three times invites drift; a one-line helper on `KeyModSet` is cheap and self-documenting.
- **Crossterm #986 acceptance.** Shift modifier is dropped on Windows 11 mouse events (documented in the PLAN.md "Edge Cases" section). On Windows 11, Shift+wheel will degrade to plain wheel — the user keeps keyboard PageUp/PageDown. No Phase 2 task tries to detect or work around this.
- **No coordinate gating.** Phase 2 deliberately ignores `(x, y)` and routes scroll by `UiMode` only (per the plan: "Test that scroll events outside the log area still scroll — we intentionally don't gate on coordinate for v1"). Phase 3+ introduces region-based hit-testing for clicks; scroll routing remains coordinate-free unless a future requirement surfaces.
- **No new `Message` variants.** Every Phase 2 routing target already exists in `message.rs`. If a per-mode scroll has no existing message (e.g. `DevTools / Performance`, `Settings` editing), Phase 2 returns `None` rather than inventing a new variant. Phase 3+ may add coordinate-aware messages (`FocusLogEntryAtRow`, `SelectFrameAtX`, etc.) for click handling.
- **Tag-filter overlay is a Normal-mode sub-state.** `TagFilter` is not a `UiMode` variant — it is `state.tag_filter_visible`, intercepting input inside `UiMode::Normal`. The `handler/mouse/normal.rs` submodule must check this flag first, mirroring the keyboard handler at `keys.rs:105-126`.
- **DevTools / Performance is intentionally a scroll no-op.** The Performance panel's frame timeline is keyboard-Left/Right only (`keys.rs:568-579`); there is no up/down scroll concept. Inserting one would require a new `Message` variant for unclear gain.
- **No `is_busy` gate for scroll.** The keyboard scroll handler comments: "always allowed" (`keys.rs:263`). Phase 2 follows suit — scroll is never blocked by reload state.
- **Naming.** `Press` (not `Click`) per the Phase 1.5 rename. Phase 2 task descriptions assume `MouseInput::Press`. If Phase 1.5 has not landed when an implementor picks up a Phase 2 task, the implementor must either (a) wait, or (b) substitute `Click` and add a TODO to switch when the rename lands.
