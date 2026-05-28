# Phase 3: Spinner in More States — Task Index

## Overview

Use the existing braille throbber **consistently wherever the UI is waiting**, not just on the startup loading screen. Today the only spinner is an inline `SPINNER` constant in `render_loading_screen` (`render/mod.rs:523`), driven by `LoadingState::animation_frame`. The new-session dialog shows a frozen "Discovering devices…" line and a static refresh glyph in the tab bar.

This phase extracts a pure, reusable `widgets/spinner.rs` helper (mirroring the Phase 2 `widgets/shimmer.rs` model) keyed off a `u64` frame, swaps the loading screen onto it with **zero visual change**, then animates the dialog's discovery/refresh states using the global `AppState::animation_frame` added in Phase 0.

**Total Tasks:** 3
**Estimated Hours:** 2–3h

## Background (confirmed by research)

- `AppState::animation_frame: u64` already exists and advances `wrapping_add(1)` on every `Message::Tick` in **all** UI modes (`handler/update.rs:92`, `state.rs:1065`). Phase 0 shipped this.
- `LoadingState::animation_frame: u64` also advances +1 per tick (`state.rs:1088`) and the loading screen renders `SPINNER[frame % SPINNER.len()]` — a direct modulo at the 50 ms tick cadence.
- `widgets/shimmer.rs` is the template to follow: a **pure** module (color/index math only, no `AppState`, no I/O), registered in `widgets/mod.rs`, with inline `#[cfg(test)] mod tests`.
- The dialog widget `NewSessionDialog` is constructed in `render/mod.rs:251` and currently receives no frame. `TargetSelector::render_loading` (`target_selector.rs:319`) prints the static discovery line; `TabBar` (`tab_bar.rs:100`) prints a static refresh icon when `refreshing`/`bootable_refreshing`.

## Task Dependency Graph

```
┌─────────────────────────────┐
│ 01-spinner-helper           │  (foundation — pure, no consumers)
└───────────────┬─────────────┘
                ▼
┌─────────────────────────────┐
│ 02-loading-screen-spinner    │  (depends 01; writes render/mod.rs)
└───────────────┬─────────────┘
                ▼
┌─────────────────────────────┐
│ 03-dialog-discovery-spinner  │  (depends 01; also writes render/mod.rs)
└─────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-spinner-helper](tasks/01-spinner-helper.md) | ✅ Done | - | 0.5–1h | `widgets/spinner.rs` (new), `widgets/mod.rs` |
| 2 | [02-loading-screen-spinner](tasks/02-loading-screen-spinner.md) | ✅ Done | 1 | 0.5h | `render/mod.rs` |
| 3 | [03-dialog-discovery-spinner](tasks/03-dialog-discovery-spinner.md) | ✅ Done ⚠️ | 1 | 1–1.5h | `render/mod.rs`, `widgets/new_session_dialog/mod.rs`, `target_selector.rs`, `tab_bar.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-spinner-helper | `crates/fdemon-tui/src/widgets/spinner.rs` (new), `crates/fdemon-tui/src/widgets/mod.rs` | `crates/fdemon-tui/src/widgets/shimmer.rs` (pattern reference) |
| 02-loading-screen-spinner | `crates/fdemon-tui/src/render/mod.rs` | `crates/fdemon-tui/src/widgets/spinner.rs`, `crates/fdemon-tui/src/render/snapshots/*loading.snap` |
| 03-dialog-discovery-spinner | `crates/fdemon-tui/src/render/mod.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs` | `crates/fdemon-tui/src/widgets/spinner.rs`, `crates/fdemon-app/src/state.rs` (reads `animation_frame`) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 02 | None (02 only *reads* `spinner.rs`) | Sequential (02 depends on 01) |
| 01 + 03 | None (03 only *reads* `spinner.rs`) | Sequential (03 depends on 01) |
| 02 + 03 | `crates/fdemon-tui/src/render/mod.rs` | **Sequential (same branch)** — both edit `render/mod.rs` (02 in `render_loading_screen`, 03 at the `NewSessionDialog::new` call site). Run 02 then 03 on the same branch; do **not** run in parallel worktrees. |

**Waves:** Strictly linear — Wave 1 = `01`, Wave 2 = `02`, Wave 3 = `03`. There is no intra-wave parallelism: `02` and `03` cannot compile until `spinner.rs` exists (depend on 01) and both write `render/mod.rs` (must be sequential w.r.t. each other). This mirrors Phase 2's linear chain.

**Cross-phase note:** `spinner.rs` is a brand-new file with no overlap against any other unit. The Phase 2.5 launch-lifecycle shimmer (already merged) and the Phase 2 `shimmer.rs` are untouched. No managed-doc (`ARCHITECTURE.md` / `CODE_STANDARDS.md` / `DEVELOPMENT.md`) changes are required — `spinner.rs` is an internal widget helper analogous to the already-documented `shimmer.rs` pattern; no doc-maintainer task is needed.

## Success Criteria

Phase 3 is complete when (from PLAN.md):

- [ ] A new `widgets/spinner.rs` exposes the braille frame set and a pure `spinner_char(frame: u64) -> char` that advances deterministically and wraps cleanly (no panic near `u64::MAX`), unit-tested.
- [ ] The startup loading screen uses the shared spinner with **no visual regression** (same glyphs, same cadence; the existing `*loading.snap` snapshot still matches without edits, or is re-blessed only if byte-identical).
- [ ] The new-session dialog's "Discovering devices…" line shows an animated spinner; the tab-bar refresh indicator shows motion instead of a frozen glyph.
- [ ] Concurrent dialog spinners are computed from a single frame value per render so they pulse **in phase**.
- [ ] `cargo test -p fdemon-tui`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Orchestration Status

**Completed 2026-05-28.** All 3 tasks done on branch `feat/ux-polish-and-multilaunch` (strictly linear, same branch — no worktrees). Validation: 01 PASS, 02 PASS (loading snapshot byte-identical, no visual regression), 03 PASS with one concern.

⚠️ **Open concern (task 03, non-blocking):** Replacing the static `icons.refresh()` glyph in `TabBar` with the animated spinner left the `TabBar.icons` field dead; it is retained with a narrowly-scoped `#[allow(dead_code)]` for API stability (15 `TabBar::new()` call sites). Follow-up: either drop the `icons` parameter from `TabBar::new()` (and update call sites) or give `icons` a real use within `TabBar`. Functional and clippy-clean as-is.

## Notes / Scope Decisions

- **Spinner cadence split (deliberate):** `spinner_char(frame)` is pure direct-modulo over the frame set. The **loading screen** passes `loading.animation_frame` unchanged → byte-identical to today (satisfies "no visual regression"). The **dialog** derives a calmer index `animation_frame / SPINNER_TICKS_PER_FRAME` (named constant, ~100 ms/frame) so the discovery spinner reads as deliberate rather than frantic at the 20 fps tick. Both behaviors live behind the one pure helper; the divisor is chosen by the caller. See task 01 for the constant's derivation comment.
- **"In phase" requirement:** task 03 computes the dialog frame index **once** at the top of the relevant render path and threads the same value to both the discovery line and the tab-bar refresh glyph.
- **Out of scope (PLAN "Optional"):** spinner glyphs for main-view `Reloading`/connecting session rows. The Phase 2 / 2.5 work already shimmers the `Reloading`/`Launching`/`Preparing` status labels; layering a spinner glyph onto the same status surface risks visual conflict and is deferred to Future Enhancements. This phase is strictly: shared helper + loading screen + dialog discovery/refresh.
- **No config / keybindings changes.** A configurable spinner speed and an "animations off" accessibility toggle are PLAN Future Enhancements.
- **Threading the global frame:** task 03 adds an `animation_frame: u64` field + builder to `NewSessionDialog`, threaded through to `TargetSelector` and `TabBar`. Default it to `0` so existing widget unit tests that construct these types directly need no signature churn beyond the new builder (prefer a `.animation_frame(u64)` builder over a required `new` parameter to minimize test breakage).
