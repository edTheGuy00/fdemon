# Phase 2: Shimmer Status Text — Task Index

## Overview

Add a subtle left-to-right color sweep ("shimmer") to **transient** status labels so the UI signals "work happening" while a session is starting, reloading, or stopping. A new pure `widgets/shimmer.rs` helper (RGB `lerp` + moving-head phase + per-character span builder) is driven by the global `AppState::animation_frame` added in Phase 0. The only consumer in this phase is the bottom metadata bar's phase label (`render_bottom_metadata`); steady states (`Running`/`Stopped`) and all keybinding-bearing text stay static.

**Total Tasks:** 2
**Estimated Hours:** 2–3h

## Task Dependency Graph

```
┌─────────────────────────────┐
│ 01-shimmer-helper           │  (foundation — pure, no consumers)
└───────────────┬─────────────┘
                ▼
┌─────────────────────────────┐
│ 02-apply-shimmer-status      │  (depends 01)
└─────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-shimmer-helper](tasks/01-shimmer-helper.md) | ✅ Done | - | 1–1.5h | `widgets/shimmer.rs`, `widgets/mod.rs` |
| 2 | [02-apply-shimmer-status](tasks/02-apply-shimmer-status.md) | ✅ Done | 1 | 1–1.5h | `widgets/log_view/mod.rs`, `render/mod.rs`, `widgets/log_view/tests.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-shimmer-helper | `crates/fdemon-tui/src/widgets/shimmer.rs` (new), `crates/fdemon-tui/src/widgets/mod.rs` | `crates/fdemon-tui/src/theme/palette.rs` |
| 02-apply-shimmer-status | `crates/fdemon-tui/src/widgets/log_view/mod.rs`, `crates/fdemon-tui/src/render/mod.rs`, `crates/fdemon-tui/src/widgets/log_view/tests.rs` | `crates/fdemon-tui/src/widgets/shimmer.rs`, `crates/fdemon-tui/src/theme/styles.rs`, `crates/fdemon-app/src/state.rs` |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 02 | None (02 only *reads* `shimmer.rs`) | Sequential (02 depends on 01) |

**Waves:** Wave 1 = `01`. Wave 2 = `02`. Strictly sequential — task 02 cannot compile until `shimmer.rs` exists and exports its public API. Only two tasks, so no intra-wave parallelism.

**Cross-phase note:** `shimmer.rs` is intentionally written as a reusable helper. Phase 6 (reload success flash) reuses its `lerp_color`; Phase 3 (spinner) is independent. Neither writes the files this phase touches.

## Success Criteria

Phase 2 is complete when:

- [x] A new `widgets/shimmer.rs` exposes a pure RGB `lerp_color`, a `shimmer_phase(frame)` that wraps over a fixed period, and a span builder that tints each character's fg between a base and a highlight color based on distance from a moving head.
- [x] `lerp_color`, `shimmer_phase`, and the span builder have unit tests (endpoint colors, phase wrap, empty/short text, non-RGB graceful fallback).
- [x] The bottom metadata bar's phase label shimmers **only** while the session is in a transient phase (`Initializing`, `Reloading`, `Quitting`, or `is_busy`); `Running`/`Stopped` render the existing static style with no visual change.
- [x] The shimmer is driven by `AppState::animation_frame` (threaded through `StatusInfo`), so it advances with the existing 50 ms tick loop and needs no new timer.
- [x] Bold/other modifiers on the phase style are preserved under shimmer; only fg color is animated.
- [x] `cargo test -p fdemon-tui`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Notes

- **Scope:** Only the bottom metadata bar phase label. The header (`header.rs::render_title_row`) renders a single status-dot *glyph*, not a label — shimmering one character is not a sweep, so it stays out of scope (noted in PLAN Future Enhancements territory).
- **Frame source:** Use `AppState::animation_frame` (global, advances in every `UiMode`), **not** `LoadingState::animation_frame` (loading-screen-only).
- **Compute phase once per render** so the sweep is coherent across the whole label.
- **Non-RGB fallback:** `lerp_color` must degrade gracefully when a color is not `Color::Rgb` (return the base color) — ratatui/crossterm down-convert RGB on 256-color terminals automatically.
- **No config / keybindings / managed-doc changes** in this phase; an "animations off" accessibility toggle and configurable speed are PLAN Future Enhancements.
