# Phase 1: Foundation — Task Index

## Overview

Phase 1 lays down the plumbing for mouse support without changing user-visible behavior. Mouse events flow from the terminal through a new abstract `MouseInput` type onto the existing TEA message bus, are silently consumed by a no-op `handle_mouse` dispatcher, and the entire feature is gated behind a new `[ui] enable_mouse` setting (default `true`). Mouse-capture enable/disable runs at the right points in the runner lifecycle and through the panic hook, with an `AtomicBool` guard against the crossterm #613 disable-without-enable Windows panic.

When Phase 1 is done, a user can scroll/click anywhere in fdemon and nothing changes — but the terminal is never left in a broken state on crash, and `enable_mouse = false` truly disables capture (no escape sequences emitted). Phases 2+ rewrite `handle_mouse` to do real work.

**Total Tasks:** 6
**Estimated Hours:** 7–11 hours

## Task Dependency Graph

```
┌─────────────────────────────────┐  ┌────────────────────────────────┐  ┌────────────────────────────────┐
│ 01 - input-mouse-type           │  │ 03 - enable-mouse-setting      │  │ 05 - mouse-capture-lifecycle   │
│ (fdemon-app/input_mouse.rs)     │  │ (config/types + settings_items)│  │ (fdemon-tui/terminal.rs)       │
└─────────────┬───────────────────┘  └────────────────────────────────┘  └─────────────┬──────────────────┘
              │                                                                        │
              ▼                                                                        ▼
┌─────────────────────────────────┐                                       ┌────────────────────────────────┐
│ 02 - message-and-handler-shell  │                                       │ 06 - wire-runners              │
│ (Message::Mouse + handle_mouse) │                                       │ (runner.rs enable/disable)     │
└─────────────┬───────────────────┘                                       └────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────┐
│ 04 - tui-event-conversion       │
│ (fdemon-tui/event.rs)           │
└─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate |
|---|------|--------|------------|------------|-------|
| 1 | [01-input-mouse-type](tasks/01-input-mouse-type.md) | Done ⚠️ | — | 1–2h | `fdemon-app` |
| 2 | [02-message-and-handler-shell](tasks/02-message-and-handler-shell.md) | Done | 1 | 1–2h | `fdemon-app` |
| 3 | [03-enable-mouse-setting](tasks/03-enable-mouse-setting.md) | Done | — | 1h | `fdemon-app` |
| 4 | [04-tui-event-conversion](tasks/04-tui-event-conversion.md) | Done | 1, 2 | 1–2h | `fdemon-tui` |
| 5 | [05-mouse-capture-lifecycle](tasks/05-mouse-capture-lifecycle.md) | Done | — | 2–3h | `fdemon-tui` |
| 6 | [06-wire-runners](tasks/06-wire-runners.md) | Done | 5 | 1h | `fdemon-tui` |

> **Concern (Task 01):** `cargo clippy -p fdemon-app --all-targets -- -D warnings` fails with three `assertions_on_constants` errors in `crates/fdemon-app/src/input_mouse.rs` lines 182–184 (e.g. `assert!(!KeyModSet::NONE.shift)`). Surfaced during Task 02 validation. Must be fixed before Phase 1 success criteria are met (replace `assert!` on consts with `assert_eq!` against runtime-bound copies, or `#[allow(clippy::assertions_on_constants)]` on the test module).

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-input-mouse-type | `crates/fdemon-app/src/input_mouse.rs` (NEW), `crates/fdemon-app/src/lib.rs` | `crates/fdemon-app/src/input_key.rs` (reference pattern) |
| 02-message-and-handler-shell | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/handler/mouse.rs` (NEW), `crates/fdemon-app/src/handler/update.rs` | `crates/fdemon-app/src/input_mouse.rs` (Task 01), `crates/fdemon-app/src/handler/keys.rs` (reference pattern), `crates/fdemon-app/src/state.rs` |
| 03-enable-mouse-setting | `crates/fdemon-app/src/config/types.rs`, `crates/fdemon-app/src/settings_items.rs`, `crates/fdemon-app/src/handler/settings.rs` | `crates/fdemon-app/src/config/types.rs` (UiSettings struct) |
| 04-tui-event-conversion | `crates/fdemon-tui/src/event.rs` | `crates/fdemon-app/src/input_mouse.rs` (Task 01), `crates/fdemon-app/src/message.rs` (Task 02) |
| 05-mouse-capture-lifecycle | `crates/fdemon-tui/src/terminal.rs` | — |
| 06-wire-runners | `crates/fdemon-tui/src/runner.rs` | `crates/fdemon-tui/src/terminal.rs` (Task 05), `crates/fdemon-app/src/config/types.rs` (Task 03 — reads `settings.ui.enable_mouse`) |

### Overlap Matrix

Wave 1 (no dependencies): 01, 03, 05
Wave 2 (after wave 1): 02, 06
Wave 3 (after wave 2): 04

| Task Pair | Same Wave? | Shared Write Files | Isolation Strategy |
|-----------|------------|--------------------|---------------------|
| 01 + 03 | Wave 1 | None | **Parallel (worktree)** |
| 01 + 05 | Wave 1 | None | **Parallel (worktree)** |
| 03 + 05 | Wave 1 | None | **Parallel (worktree)** |
| 02 + 06 | Wave 2 | None | **Parallel (worktree)** |
| 02 + 04 | Different waves (04 depends on 02) | n/a | Sequential by dependency |
| 04 + 06 | Different waves | n/a | Sequential by dependency |

All wave-peer task pairs have zero shared write files — Phase 1 is fully parallelizable within each wave.

## Success Criteria

Phase 1 is complete when:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes — all new tests green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] CI green on Linux + macOS + Windows
- [ ] At minimum these new tests exist and pass:
  - `MouseInput` enum constructibility, equality, debug-format
  - Each `crossterm::MouseEventKind` variant maps to the expected `MouseInput` (or `None` for `Moved`)
  - `KeyModifiers` → `KeyModSet` round-trip preserves Shift / Ctrl / Alt
  - `disable_mouse_capture()` is a no-op when `enable_mouse_capture()` was never called (asserted via the `AtomicBool` guard, since direct stdout assertion is impractical)
  - `handle_mouse` returns `None` for every `UiMode` variant given any `MouseInput`
  - `update(state, Message::Mouse(...))` returns `UpdateResult::none()` and does not mutate state
  - `apply_project_setting` correctly toggles `settings.ui.enable_mouse`
- [ ] Manual smoke test on macOS:
  - Start fdemon → click anywhere → no behavior change, no crash
  - Set `enable_mouse = false` in `.fdemon/config.toml` → restart → wheel scrolls native terminal scrollback (capture not engaged)
  - Press Ctrl+C while running → terminal is fully usable afterward (cursor visible, mouse not stuck on, raw mode off)
  - Trigger a panic via a debug `panic!` in `run_loop` (manual local test) → terminal is fully usable afterward
- [ ] No `Message::Mouse` variant is wired to any visible behavior yet (Phase 2 does that)

## Notes

- **No new external dependencies.** Crossterm 0.29 already provides everything; ratatui 0.30 is unchanged. Workspace `Cargo.toml` does not need editing.
- **Default = `enable_mouse: true`.** Power users who want native shell-style text selection without `Shift+drag` can set it to `false` once they discover it via the settings panel or `docs/CONFIGURATION.md`. Most modern terminals natively pass `Shift+drag` through when capture is on, so the default does not break selection for most users.
- **`selector.rs` is intentionally untouched.** The project selector runs before the engine exists (no settings yet) and is short-lived; mouse there is a Phase 5 stretch goal, not a Phase 1 concern.
- **`runner::run()` (demo entry, no project)** also intentionally skips mouse enable in Phase 1 — there is no Settings to read. Phase 6 documentation will mention this if it becomes user-visible; for now it is internal-only.
- **Crossterm #613 guard.** Task 05's `AtomicBool` is the load-bearing safety: without it, calling `disable` on a Windows machine where `enable` failed (e.g., legacy conhost) panics with `TryFromIntError`. This is verified by a unit test that calls `disable` first without ever calling `enable`.
- **Crossterm #986 acceptance.** Shift-modifier-on-mouse-events is broken on Windows 11 today. Phase 1 makes no use of Shift-on-mouse, so this is dormant; Phase 2 documents the degradation when Shift+wheel is introduced.
- **TEA purity.** Phase 1 introduces no `Cell` or render-time mutation. The `Cell<MouseRegions>` exception is deferred to Phase 3 when widgets actually need to register clickable rects.
