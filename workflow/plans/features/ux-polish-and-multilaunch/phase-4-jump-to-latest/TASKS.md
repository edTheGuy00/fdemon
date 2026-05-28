# Phase 4: Jump-to-Latest Log Affordance — Task Index

## Overview

Make "jump to the live tail" **discoverable** and tell the user when they've fallen behind the stream. Today `G` / `End` already re-engage tail-follow (`handler/keys.rs:308–316` → `Message::ScrollToBottom` → `handle_scroll_to_bottom` → `LogViewState::scroll_to_bottom`), but there is no visual feedback that logs are accumulating off-screen.

This phase adds a per-`Session` `unseen_log_count` counter that increments while the user is scrolled up, and a floating right-aligned `↓ N new · G to jump` indicator inside the log view that hides whenever auto-scroll is active. Closes issue #31.

**Total Tasks:** 2
**Estimated Hours:** 2–3h

## Background (confirmed by research)

- All log + scroll state is fully per-session (`Session.logs: VecDeque<LogEntry>`, `Session.log_view_state: LogViewState`). `AppState` has **no** legacy log fields. `state.session_manager.selected_mut()` gives the active session in both handlers and render.
- `LogViewState.auto_scroll: bool` is the canonical "following the tail" flag (`log_view_state.rs:45–66`). Default `true`. Set `false` by any upward scroll (`scroll_up`, `scroll_to_top`). Restored `true` by `scroll_to_bottom()` (line 137–140) **and** by `scroll_down()` reaching the natural bottom (line 120–128).
- `Session::add_log` (`session/session.rs:277–362`) is the single funnel for all log arrivals — `add_logs_batch`, `queue_log`, and `flush_batched_logs` all loop through `add_log`. Ring-buffer eviction at line 339–361 adjusts `log_view_state.offset` (decrement on each evicted entry) but is independent of the new counter.
- Render flow: `render::view` → `widgets::LogView::new(&handle.session.logs, icons)` (`render/mod.rs:178`) → builder chain → `widgets::log_view::render_with_regions(area, buf, &mut handle.session.log_view_state, log_view, log_ctx)`. The builder pattern (`filter_state`, `wrap_mode`, `search_state`, `with_status`, `link_highlight_state`) is the seam for adding `unseen_log_count`.
- `render_inner` already does right-aligned `buf.set_line` overlays (`render_metadata_bar` for the `[LIVE FEED]` badge, `render_bottom_metadata` for the status row). The pill follows the same pattern at `content_area.y + content_area.height - 1`.
- `MouseCtx` is fully wired through `render_with_regions` (the `log_ctx: Option<&mut MouseCtx<'_>>` parameter). Existing log-row clicks register `Message::ClickLogRow` at z=0; the pill registers `Message::ScrollToBottom` at z=0 the same way.
- **No new `Message` variant is needed.** `G`/`End` and the pill click both emit the existing `Message::ScrollToBottom`.
- **No new keybinding is needed.** This phase makes the existing `G`/`End` binding discoverable, not new.

## Task Dependency Graph

```
┌─────────────────────────────────┐
│ 01-unseen-log-count-state       │  (foundation; fdemon-app only)
└────────────────┬────────────────┘
                 ▼
┌─────────────────────────────────┐
│ 02-log-view-indicator-render    │  (depends 01; fdemon-tui only)
└─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-unseen-log-count-state](tasks/01-unseen-log-count-state.md) | Not Started | - | 0.5–1h | `session/session.rs`, `handler/scroll.rs` |
| 2 | [02-log-view-indicator-render](tasks/02-log-view-indicator-render.md) | Not Started | 1 | 1.5–2h | `widgets/log_view/mod.rs`, `widgets/log_view/tests.rs`, `render/mod.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-unseen-log-count-state | `crates/fdemon-app/src/session/session.rs`, `crates/fdemon-app/src/handler/scroll.rs` | `crates/fdemon-app/src/log_view_state.rs` (reads `auto_scroll` + transition semantics) |
| 02-log-view-indicator-render | `crates/fdemon-tui/src/widgets/log_view/mod.rs`, `crates/fdemon-tui/src/widgets/log_view/tests.rs`, `crates/fdemon-tui/src/render/mod.rs` | `crates/fdemon-app/src/session/session.rs` (reads new `unseen_log_count` field), `crates/fdemon-app/src/message.rs` (reads `Message::ScrollToBottom` — existing), `crates/fdemon-tui/src/widgets/log_view/styles.rs` (color tokens) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|--------------------|
| 01 + 02 | None (02 only *reads* `session.rs`) | **Sequential (same branch)** — 02 cannot compile until 01 adds `Session::unseen_log_count`. Run 01 then 02 on the same branch. |

**Waves:** Strictly linear — Wave 1 = `01`, Wave 2 = `02`. No intra-wave parallelism: 02 depends on a field that does not yet exist before 01. Different crates so no actual write-file overlap, but the compile dependency makes worktree parallelism pointless.

**Cross-phase note:** Phase 4 is independent of every other phase in this feature (`PLAN.md` "Delivery" section explicitly calls out "Log-indicator unit — Phase 4. Fully independent."). It can ship before, after, or in parallel with the animation and multi-launch units. No managed-doc (`ARCHITECTURE.md` / `CODE_STANDARDS.md` / `DEVELOPMENT.md`) changes are required — the change adds one `Session` field and a render overlay, neither of which warrants architecture documentation. `KEYBINDINGS.md` already documents `G`/`End`; this phase only makes the existing binding discoverable.

## Success Criteria

Phase 4 is complete when (from PLAN.md):

- [ ] Scrolling up during a streaming log stream shows a `↓ N new · G to jump` indicator near the bottom-right of the log area; the indicator hides as soon as the view is following the tail.
- [ ] `unseen_log_count` increments **only** while `!log_view_state.auto_scroll`, and resets to 0 whenever auto-scroll is re-engaged — covered for both the `G`/`End` jump path **and** the natural follow-on-`scroll_down`-to-bottom path. Unit-tested at both the `Session` and scroll-handler levels.
- [ ] Pressing `G` or `End` jumps to the tail, re-enables auto-scroll, **and** clears the indicator within one render frame.
- [ ] Display gracefully caps at `↓ 999+ new · G to jump`; pill is suppressed when `content_area.width` is below the pill's minimum width (narrow-terminal fallback).
- [ ] Clicking the pill emits the existing `Message::ScrollToBottom` (no new message variant added).
- [ ] `cargo test --workspace`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Keyboard Shortcuts

This phase introduces **no new keybindings**. It advertises the existing ones:

| Key | Context | Action |
|-----|---------|--------|
| `G` / `End` | Log view | Jump to latest & follow (existing — now advertised by the pill) |

## Notes / Scope Decisions

- **Pill label is `↓ N new · G to jump`** (middle-dot separator, decided over `—`/em-dash for narrower terminals). PLAN.md draft text used em-dash; we override here for width.
- **Cap at `999+`** to keep the pill width bounded. Named constant `JUMP_HINT_MAX_DISPLAY = 999` in `widgets/log_view/mod.rs`. Past 999 the exact count is advisory only — PLAN risks section already calls this out.
- **Reset on natural follow re-engagement** is wired in the **handler** (`handle_scroll_down`), not inside `LogViewState`. Captures pre-state `auto_scroll`, calls `mark_tail_followed()` after the scroll if the flag transitioned `false → true`. Keeps `LogViewState` ignorant of `Session` (no circular-knowledge problem).
- **Ring-buffer eviction does not decrement the counter.** Evicted entries are old; unseen entries are new (just pushed to the back). The counter represents "new arrivals while away," not a buffer position. Unit-tested.
- **`saturating_add`** on the counter prevents overflow at `usize::MAX`. Tested.
- **No mouse click required for completeness** — the pill is primarily visual. Mouse routing is added because `MouseCtx` is already wired and the cost is one line. Out of scope: keyboard focus / cursor on the pill.
- **Out of scope:** configurable pill format, "animations off" accessibility toggle, status-bar-level "you have new logs" badge outside the log view area. These belong in Future Enhancements per PLAN.md.
- **Threading the field through `LogView` uses a `.unseen_log_count(u64)` builder** so the existing widget unit tests that construct `LogView` directly need no signature churn (default 0).
