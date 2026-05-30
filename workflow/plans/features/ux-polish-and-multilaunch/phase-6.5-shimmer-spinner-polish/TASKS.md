# Phase 6.5: Shimmer Timing Polish + Launch Spinner Icon — Task Index

## Overview

Two small, independent TUI-only follow-ups after Phases 2 / 2.5 / 3 shipped:

1. **Shimmer sweep refinement** — the status-label shimmer (`widgets/shimmer.rs`
   `shimmer_spans`) "feels off": the bright head **pops in** at the first character
   and **snaps back** every cycle with no rest, reading as a constant mechanical
   sweep. A smoother variant maps the head off-screen at both ends
   (`head = phase * (n + 6) − 3` instead of `phase * n`) so it fades in from the
   left, exits off the right, and leaves a brief all-dim **rest gap** between
   sweeps. The period (~1.5 s, frame counter `% 30`) is **unchanged** — only the
   spatial mapping and the falloff width (`4.0 → 3.5`) change. Pure math in one
   function; every call site benefits with no call-site edit.

2. **Launch spinner icon** — the launch-lifecycle phases (`Initializing`,
   `Preparing`, `Launching`) currently render a **static** `○` icon next to their
   shimmering label in the bottom status bar. Swap that icon for the existing
   braille spinner (`widgets/spinner.rs`), keyed off the global `animation_frame`
   at the same `SPINNER_TICKS_PER_FRAME` cadence the dialog discovery spinner uses,
   so the in-progress glyph animates in unison. Scope is **bottom status bar only**
   (`widgets/log_view/mod.rs` `render_bottom_metadata`); the header title row and
   session tabs keep their static phase icons. `Reloading` (`↻`), `Quitting`
   (`✗`), `Running` (`●`), and `Stopped` (`○`) keep their static icons.

Both tasks touch different files in the same crate (`fdemon-tui`) and have no
dependency on each other.

**Total Tasks:** 2
**Estimated Hours:** 1.5–2.5h

## Task Dependency Graph

```
┌─────────────────────────────────────┐   ┌─────────────────────────────────────┐
│ 01-shimmer-sweep-refinement          │   │ 02-launch-spinner-icon               │
│   fdemon-tui/widgets/shimmer.rs      │   │   fdemon-tui/widgets/log_view/mod.rs │
│   (pure-math head range + falloff)   │   │   (spinner glyph for launch phases)  │
└─────────────────────────────────────┘   └─────────────────────────────────────┘
                 (independent — run in parallel)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-shimmer-sweep-refinement](tasks/01-shimmer-sweep-refinement.md) | ✅ Done | - | 1–1.5h | `crates/fdemon-tui/src/widgets/shimmer.rs` |
| 2 | [02-launch-spinner-icon](tasks/02-launch-spinner-icon.md) | ✅ Done | - | 0.5–1h | `crates/fdemon-tui/src/widgets/log_view/mod.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-shimmer-sweep-refinement | `crates/fdemon-tui/src/widgets/shimmer.rs` | — |
| 02-launch-spinner-icon | `crates/fdemon-tui/src/widgets/log_view/mod.rs` | `crates/fdemon-tui/src/widgets/spinner.rs` (`spinner_char`, `SPINNER_TICKS_PER_FRAME` — read), `crates/fdemon-core` (`AppPhase`), `crates/fdemon-tui/src/theme/styles.rs` (`phase_indicator`) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 02 | None | Parallel (worktree) |

**Waves:** Wave 1 = `01` + `02` together. Disjoint write files, no dependency edge,
safe to run concurrently in isolated worktrees.

**Cross-phase note:** Task 01 modifies `shimmer.rs` only (the `shimmer_spans` head
math). Phase 6 reuses `lerp_color` from the same file but does **not** modify it and
is already merged, so there is no live conflict. Task 02 only *reads* `spinner.rs`
(unchanged since Phase 3) and consumes the existing `phase_indicator` mapping.

## Success Criteria

Phase 6.5 is complete when:

- [x] The status-label shimmer sweeps in from off-screen, exits off-screen, and has
      a visible rest gap between cycles (no pop-in/snap-back); the change is confined
      to `shimmer_spans` and shared by all call sites with no call-site edit.
- [x] `shimmer_spans` unit tests are updated for the new sweep range and `3.5`
      falloff and pass (the existing `shimmer_spans_head_is_brightest` assertions
      change under the new range — re-derive them).
- [x] The bottom status bar shows the braille spinner in place of the static icon
      for `Initializing`, `Preparing`, and `Launching` only; `Reloading`, `Quitting`,
      `Running`, and `Stopped` keep their static icons (incl. the `is_busy` path).
- [x] The status-bar spinner advances in unison with the new-session dialog spinner
      (same `SPINNER_TICKS_PER_FRAME` divisor off the global `animation_frame`).
- [x] The label shimmer (`is_transient`) behaviour is unchanged — only the leading
      glyph changes for the three launch phases.
- [x] `cargo test --workspace`, `cargo fmt --all -- --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Notes

- **No new state, no new config, no keybindings.** Both tasks consume the existing
  global `AppState::animation_frame` (50 ms / 20 fps tick) already plumbed into
  `StatusInfo::animation_frame`. A configurable shimmer/spinner speed and an
  "animations off" accessibility toggle remain PLAN Future Enhancements.
- **No managed-doc change.** No `AppPhase` / `Message` / module-structure change,
  so `docs/ARCHITECTURE.md`, `docs/CODE_STANDARDS.md`, and `docs/DEVELOPMENT.md`
  need no update for this phase.
- **Spinner phase set is deliberately narrow.** `Reloading` and `Quitting` retain
  their semantically meaningful glyphs (`↻`, `✗`); only the launch-lifecycle family
  (`Initializing`/`Preparing`/`Launching`), which has no distinctive glyph of its
  own (all `○`), gains the spinner.
- **Shimmer change is spatial only.** Keep the frame-counter phase source
  (`shimmer_phase(frame)`, period 30) so all concurrent shimmers stay in unison; do
  not switch to a per-widget wall-clock phase (it would desync separate labels).
