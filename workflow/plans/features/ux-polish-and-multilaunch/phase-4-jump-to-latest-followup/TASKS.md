# Phase 4 Follow-up: Jump-to-Latest Review Fixes — Task Index

## Overview

Address the code-review findings for Phase 4 (Jump-to-Latest Log Affordance), recorded in
`workflow/reviews/features/ux-polish-and-multilaunch/phase-4-jump-to-latest/ACTION_ITEMS.md`.

The original phase shipped a per-`Session` `unseen_log_count` counter and a `↓ N new · G to jump`
pill. Review (5 agents + orchestrator verification) surfaced **3 MAJOR** issues and several minor
items. This follow-up closes all of them.

**Total Tasks:** 2
**Estimated Hours:** 2–3h

## Findings Addressed

| ID | Severity | Summary | Task |
|----|----------|---------|------|
| M1 | MAJOR | Pill mouse-click is shadowed by the log-row `ClickLogRow` region (equal z=0, last-push-wins) → click emits `ClickLogRow`, not `ScrollToBottom`. AC 02-6 functionally unmet. | 02 |
| M2 | MAJOR | `clear_logs` does not reset `unseen_log_count` → pill shows `↓ N new` over an empty buffer. | 01 |
| M3 | MAJOR | `handle_page_down` lacks the false→true `auto_scroll` transition guard → stale counter after paging to bottom. | 01 |
| m1 | MINOR (decided: fix) | Counter increments regardless of active log filter → pill over-reports under a filter. Gate the increment on `filter_state.matches(&entry)`. | 01 |
| m2 | MINOR | `LogView::unseen_log_count` builder lacks a `///` doc comment. | 02 |
| m3 | MINOR | Raw `u16` arithmetic in pill coordinates; use `saturating_add`/`saturating_sub`. | 02 |
| m4 | MINOR | Pill constants use `\u{...}` escapes instead of literal `↓`/`·` glyphs. | 02 |
| n1 | LOW | No test covers pill + scrollbar co-rendering layout. | 02 |
| n3 | LOW | Narrow-terminal suppression lacks exact boundary tests (`width == pill_width` vs `+1`). | 02 |

(Nitpicks n2 [unicode-width], n4 [test-helper aliases], n5 [test-module location] are explicitly
out of scope — see Notes.)

## Background (confirmed by research)

- **M1 root cause:** `MouseRegions::hit_test` (`mouse_regions.rs:187-199`) resolves overlaps with
  `.max_by_key(|(i, e)| (e.z_index, *i))` — at equal `z_index` the **last-pushed** entry wins. The
  pill region is pushed from `render_jump_to_latest_pill` (called at `log_view/mod.rs:1643`, registers
  at `:1870`) *before* the per-row `ClickLogRow` regions (`:1712`). Both at `z=0`, so the row region
  wins on the pill's cell. `MouseCtx::click_at_z(rect, action, z)` already exists (`render/mod.rs:51`);
  registering the pill at `z=1` fixes precedence.
- **M3 + mouse wheel:** mouse wheel-down routes through `Message::ScrollDown` / `Message::PageDown`
  (`handler/mouse/link_highlight.rs:50,59`), which dispatch to `handle_scroll_down` / `handle_page_down`.
  So fixing the two keyboard handlers also fixes wheel-down — **no separate mouse-handler change needed.**
- **m1 feasibility:** `self.filter_state.matches(entry)` is the render-time filter predicate
  (`session.rs:711`) and is callable from `add_log` on `&self`.
- `handle_scroll_down` (`scroll.rs:19-30`) is the reference pattern: capture `was_following` before the
  scroll call, then `if !was_following && auto_scroll { mark_tail_followed() }`.

## Task Dependency Graph

```
┌─────────────────────────────────────┐   ┌─────────────────────────────────────┐
│ 01-app-state-and-handler-fixes      │   │ 02-tui-pill-click-and-cleanups      │
│ (fdemon-app: session.rs, scroll.rs) │   │ (fdemon-tui: log_view/*)            │
└─────────────────────────────────────┘   └─────────────────────────────────────┘
        (independent — different crates, disjoint files, parallel)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-app-state-and-handler-fixes](tasks/01-app-state-and-handler-fixes.md) | Not Started | - | 1–1.5h | `session/session.rs`, `handler/scroll.rs` |
| 2 | [02-tui-pill-click-and-cleanups](tasks/02-tui-pill-click-and-cleanups.md) | Not Started | - | 1–1.5h | `widgets/log_view/mod.rs`, `widgets/log_view/tests.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-app-state-and-handler-fixes | `crates/fdemon-app/src/session/session.rs`, `crates/fdemon-app/src/handler/scroll.rs` | `crates/fdemon-app/src/log_view_state.rs` (`scroll_down`/`page_down`/`auto_scroll` semantics), `crates/fdemon-app/src/session/filter_state.rs` (or wherever `FilterState::matches` lives — read for the `matches` signature) |
| 02-tui-pill-click-and-cleanups | `crates/fdemon-tui/src/widgets/log_view/mod.rs`, `crates/fdemon-tui/src/widgets/log_view/tests.rs` | `crates/fdemon-tui/src/render/mod.rs` (`MouseCtx::click_at_z`), `crates/fdemon-app/src/mouse_regions.rs` (`hit_test` precedence — read only), `crates/fdemon-tui/src/widgets/log_view/styles.rs` (pill style tokens — read only) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|--------------------|
| 01 + 02 | None — 01 writes `fdemon-app`, 02 writes `fdemon-tui`; disjoint files and crates | **Parallel (worktree)** — no write overlap. 02's pill-click fix and tests construct `LogView` with explicit counts and do not depend on 01's runtime increment changes; 01's state/handler fixes do not touch any TUI file. No compile dependency in either direction. |

**Waves:** Single wave — `01` and `02` run concurrently in isolated worktrees, then merge in task-number order.

## Success Criteria

This follow-up is complete when:

- [ ] Clicking the pill emits `Message::ScrollToBottom` — verified by a hit-test assertion at the pill cell (not mere region existence) with a log present on that row (M1).
- [ ] `clear_logs` zeroes `unseen_log_count`; the pill is not rendered over an empty buffer (M2).
- [ ] `handle_page_down` clears `unseen_log_count` iff `auto_scroll` transitions false→true (M3); mouse wheel-down (which dispatches `PageDown`/`ScrollDown`) inherits the fix.
- [ ] `add_log` increments `unseen_log_count` only for entries passing `filter_state.matches(&entry)`; the field doc comment documents filter-gating alongside the eviction note (m1).
- [ ] `LogView::unseen_log_count` builder has a `///` doc; pill coordinates use saturating arithmetic; pill constants use literal `↓`/`·` glyphs (m2, m3, m4).
- [ ] Tests cover: pill+scrollbar co-render (n1), narrow-terminal boundary at `width == pill_width` and `width == pill_width + 1` (n3).
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` all pass.

## Notes / Scope Decisions

- **Filter divergence (m1): decided to FIX, not document.** The increment is gated on
  `filter_state.matches(&entry)` so the pill counts only entries the user would actually see on jump.
  The advisory framing (eviction-independent, capped at `999+`) is preserved for the gated count.
  Note: changing the filter while scrolled up does not retroactively recompute the counter — this
  remains an accepted advisory limitation and should be stated in the field doc comment.
- **Mouse wheel-down needs no separate task** — it dispatches the same `ScrollDown`/`PageDown`
  messages that 01 fixes at the handler layer.
- **Out of scope (deferred nitpicks):** n2 (`unicode-width` for `pill_width` — acceptable for the
  fixed `↓`/`·` glyph set), n4 (trivial `make_logs`/`default_icons` test-helper aliases), n5
  (inline vs external `session` test-module location). These carry no correctness impact.
- **No managed-doc updates required.** No new modules, crates, layer/dependency changes, or new
  reusable patterns. The `unseen_log_count` field doc edit (m1) is an in-source change, not a
  `docs/ARCHITECTURE.md` / `CODE_STANDARDS.md` / `DEVELOPMENT.md` change. No `doc_maintainer` task.
- **Optional refactor (01):** extracting a shared `clear_pill_if_reengaged(handle, was_following)`
  helper for `handle_scroll_down` + `handle_page_down` is encouraged but not required; if done, keep
  the behavior identical and update both call sites.
