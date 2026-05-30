# Phase 6: Reload Success Flash — Task Index

## Overview

Give brief, positive visual feedback when a hot reload lands: tint the main header
background toward the success green and let it fade back over ~500 ms. There is no
new state and no new timer — `Session::complete_reload()` already stamps
`last_reload_time` (`session/session.rs:617`), and the existing 50 ms tick loop
already drives redraws. The work splits along the crate boundary:

1. **App layer** — a pure `Session::reload_flash_alpha(now)` helper that decays a
   `0.0→1.0` value from `last_reload_time`, returning `0.0` once the window
   elapses or the session is not in a steady `Running` phase (suppression guard).
2. **TUI layer** — `render_main_header` reads that alpha for the selected session
   and blends `CARD_BG → STATUS_GREEN` with the **existing Phase 2 `lerp_color`**
   helper before painting the header block background.

**Total Tasks:** 2
**Estimated Hours:** 1–2h

## Task Dependency Graph

```
┌─────────────────────────────────┐
│ 01-reload-flash-alpha            │  (app: pure helper + tests)
│   fdemon-app/session/session.rs  │
└────────────────┬─────────────────┘
                 ▼
┌─────────────────────────────────┐
│ 02-tint-header-flash             │  (tui: blend header bg via lerp_color)
│   fdemon-tui header + render     │
└─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-reload-flash-alpha](tasks/01-reload-flash-alpha.md) | ✅ Done | - | 0.5–1h | `crates/fdemon-app/src/session/session.rs` |
| 2 | [02-tint-header-flash](tasks/02-tint-header-flash.md) | ✅ Done | 1 | 0.5–1h | `crates/fdemon-tui/src/widgets/header.rs`, `crates/fdemon-tui/src/render/mod.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-reload-flash-alpha | `crates/fdemon-app/src/session/session.rs` | `crates/fdemon-core/src/types.rs` (`AppPhase`) |
| 02-tint-header-flash | `crates/fdemon-tui/src/widgets/header.rs`, `crates/fdemon-tui/src/render/mod.rs` | `crates/fdemon-tui/src/widgets/shimmer.rs` (`lerp_color`, existing), `crates/fdemon-tui/src/theme/palette.rs` (`CARD_BG`, `STATUS_GREEN`), `crates/fdemon-app/src/session/session.rs` (`reload_flash_alpha`) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 02 | None (02 only *reads* `session.rs`) | Sequential (02 depends on 01's API) |

**Waves:** Wave 1 = `01`. Wave 2 = `02`. Strictly sequential — task 02 cannot
compile until `reload_flash_alpha` exists and is `pub`. No intra-wave parallelism
(only two tasks, disjoint crates, single dependency edge).

**Cross-phase note:** Phase 6 deliberately reuses `lerp_color` from Phase 2's
`widgets/shimmer.rs` (already merged) rather than re-deriving RGB math. Task 02
does **not** modify `shimmer.rs`; it only imports the existing function. No file
this phase writes is touched by any other phase's open tasks.

## Success Criteria

Phase 6 is complete when:

- [x] `Session::reload_flash_alpha(now)` returns `1.0` immediately after
      `complete_reload()` and decays linearly to `0.0` over ~500 ms, then stays
      `0.0`; unit-tested at the boundaries (just-reloaded, mid-decay, expired,
      never-reloaded).
- [x] The alpha is suppressed (returns `0.0`) when the session is not in a steady
      `Running` phase — i.e. it does not bleed into `Stopped`/`Quitting`/error
      states — verified by a unit test.
- [~] A successful hot reload briefly tints the main header background toward
      `STATUS_GREEN` and fades back within ~500 ms. _(Verified by construction +
      unit tests; live-terminal visual confirmation still recommended.)_
- [x] The flash is driven entirely by `last_reload_time` + the existing tick loop
      (no new timer, no new `AppState`/`Session` field beyond the helper).
- [x] The blend reuses Phase 2's `lerp_color` (no duplicated RGB math) and
      degrades gracefully on non-RGB terminals.
- [x] `cargo test --workspace`, `cargo fmt --all -- --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Notes

- **No new state / no new timer.** `last_reload_time: Option<DateTime<Local>>`
  (`session.rs:171`) is the single source of truth. `complete_reload()` already
  sets it and flips `phase` back to `Running` (`session.rs:617`). Decay is pure
  wall-clock math against a `now` passed in by the caller.
- **`now` is a parameter, not `Local::now()` inside the helper.** Passing
  `now: DateTime<Local>` keeps the helper pure and unit-testable; `render/mod.rs`
  supplies `Local::now()` at the call site (mirrors `session_duration`'s pattern,
  but injectable). Tests construct fixed timestamps.
- **Suppression guard.** There is no dedicated "reload failed" `AppPhase` variant
  (variants: `Initializing`, `Preparing`, `Launching`, `Running`, `Reloading`,
  `Stopped`, `Quitting`). A failed reload leaves the phase at `Running` and only
  logs an error, so gate the flash on `phase == Running` — this both matches the
  "only on success" intent and suppresses the tint in `Stopped`/`Quitting`.
- **Where to tint.** `render_main_header` (`header.rs:70`) sets the block bg via
  `glass_block(false).style(Style::default().bg(palette::CARD_BG))`. Replace the
  constant `CARD_BG` with `lerp_color(CARD_BG, STATUS_GREEN, alpha * K)` where the
  blend cap `K` keeps the peak subtle. This tints the whole header (single- and
  multi-session layouts) in one place.
- **Alpha plumbing.** Compute the alpha in `render/mod.rs` from the selected
  session and pass it into `MainHeader` via a new builder (e.g.
  `.reload_flash(alpha)` defaulting to `0.0`), keeping `header.rs` free of any
  time/`Local::now()` calls. This preserves the existing `MainHeader::new`
  signature used by the header's own unit tests.
- **No config / keybindings / managed-doc changes.** A configurable flash
  duration / "animations off" toggle is PLAN Future Enhancements, not this phase.
  No `AppPhase`/`Message` additions, so no architecture-doc update is required.
