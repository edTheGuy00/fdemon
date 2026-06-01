# Phase 0: Shared Animation Frame — Task Index

## Overview

Add a single, always-incrementing animation frame counter to `AppState`, ticked on every `Message::Tick` regardless of `UiMode`. This is the time source the shimmer (Phase 2), spinner-in-more-states (Phase 3), and reload flash (Phase 6) all derive from. No user-visible change on its own.

**Total Tasks:** 1
**Estimated Hours:** 0.5–1h

## Task Dependency Graph

```
┌─────────────────────────────┐
│ 01-global-animation-frame   │  (no dependencies)
└─────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-global-animation-frame](tasks/01-global-animation-frame.md) | ✅ Done | - | 0.5–1h | `state.rs`, `handler/update.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-global-animation-frame | `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/handler/update.rs` | - |

### Overlap Matrix

Single task — no intra-phase overlap.

**Cross-phase note:** Phases 2, 3, and 6 (animation consumers, in other units) will *read* `AppState::animation_frame` and edit TUI render files; they do not write `state.rs`/`update.rs`, so this phase can land independently and ahead of them.

## Success Criteria

Phase 0 is complete when:

- [x] `AppState` has an `animation_frame: u64` field, initialized to 0.
- [x] `Message::Tick` increments it via `wrapping_add(1)` on **every** tick, irrespective of `UiMode` (including Normal, NewSessionDialog, Loading).
- [x] The existing loading-screen animation continues to work unchanged.
- [x] Unit test proves the counter advances on tick in a non-Loading mode and wraps without panicking.
- [x] `cargo test -p fdemon-app` and `cargo clippy --workspace` pass.

## Notes

- `AppState::new()` delegates to `AppState::with_settings()`, which is the only constructor that builds `Self { .. }` — initialize the field there (one site).
- Keep this counter independent of `LoadingState::animation_frame`; do not remove or repurpose the loading-screen counter.
- Consider a small accessor (`pub fn animation_frame(&self) -> u64`) for render-layer use, but a public field is also acceptable and matches existing `AppState` style.
