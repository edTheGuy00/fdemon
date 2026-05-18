# Plan: Phase 1 Follow-up Fixes

**Status:** Approved
**Driver:** [`workflow/reviews/features/devtools-performance-memory-split-phase-1/REVIEW.md`](../../../../reviews/features/devtools-performance-memory-split-phase-1/REVIEW.md) and the matching [`ACTION_ITEMS.md`](../../../../reviews/features/devtools-performance-memory-split-phase-1/ACTION_ITEMS.md)
**Parent feature:** `devtools-performance-memory-split` (see [`../PLAN.md`](../PLAN.md))
**Predecessor phase:** [`../phase-1/TASKS.md`](../phase-1/TASKS.md) — all 5 tasks merged 2026-05-18

---

## TL;DR

Phase 1 of the Performance/Memory tab split landed but post-merge review surfaced 3 Critical bugs (including one already-fixed-in-flight regression), 5 Major UX/correctness gaps, and 12 Minor cleanups. This phase bundles the rework into 5 tasks across 2 waves so Phase 1 reaches a shippable state before Phase 2 (Performance details expansion) begins.

## Problem

The shipped Phase 1 has three concrete defects:

1. **`update.rs:1920`** — `Message::VmServicePerformanceMonitoringStarted` only unpauses `alloc_pause_tx` when the active panel is `Performance`. Users with `default_panel = "memory"` see allocation polling stuck paused indefinitely.
2. **`handler/mouse/devtools.rs:99`** — mouse-wheel events on the Memory panel dispatch `PerfScroll*` messages, silently mutating `session.performance.frame_chart_scroll_offset` instead of moving the memory chart or allocation table.
3. **`update.rs:VmServicePerformanceMonitoringStarted`** — `performance.monitoring_active` was never flipped to `true` after the T02 state split, leaving the Performance tab stuck on "performance monitoring starting…" forever. **Already fixed in-flight; needs committing.**

Plus five user-visible Major gaps: missing disconnected-state UI on Memory panel, stale module docstrings, missing entries in `docs/REVIEW_FOCUS.md` for new Cell-render-hint fields, the `PerfSection::DetailsTab` "keypress trap" (Tab moves focus to an invisible dead variant), and K/M/G precision loss in allocation-table instance counts.

## Goals

1. **Restore handler correctness** — three critical bugs fixed and covered by regression tests (T01).
2. **Memory panel reaches parity with Performance** for connection-state UX, allocation-count precision, and code hygiene (T02).
3. **Remove the `DetailsTab` keypress trap** — decide between collapsing the cycle or rendering a visible Phase-2 stub. Rename to avoid the `state::DetailsTab` (Inspector) collision (T03).
4. **Documentation is in sync** with the post-split architecture (T04): `REVIEW_FOCUS.md` registers the two new Cell fields, performance widget/handler module docstrings describe the post-T03 single-section reality, the Performance footer reflects actual bindings.
5. **Eliminate the `clamp_chart_scroll` / `ScrollDir` duplication** between `handler/devtools/performance.rs` and `handler/devtools/memory.rs` (T05).

## Non-Goals

- **Phase 2 work** (Frame Analysis / Rebuild Stats / Timeline Events tabs) is unchanged by this followup and remains scheduled after.
- **No new VM Service RPCs.** All fixes operate on existing data plumbing.
- **No new keyboard shortcuts.** The `m` / `Tab` / `s` bindings stay as Phase 1 shipped.
- **No `KEYBINDINGS.md` rewrite** — the doc is already correct after Phase 1's T04; only the Performance tab *footer hint string* needs a minor update (T04).

## Approach

5 tasks across 2 waves. Wave 1 runs three independent tasks in parallel worktrees (zero write-file overlap). Wave 2 runs two sequential tasks on the working branch since they update files touched in Wave 1.

```
                    ┌────────────────────────────────┐ ┌──────────────────────────────┐ ┌────────────────────────────────┐
        Wave 1      │ 01 fix-critical-handler-bugs   │ │ 02 memory-panel-followups    │ │ 03 resolve-details-tab-trap    │
        (parallel)  │   C1 + C2 + commit C3 + m7,m8  │ │   M1 + M5 + m1,m2,m3,m5,m12  │ │   M4 + m11                     │
                    └────────────────┬───────────────┘ └──────────────┬───────────────┘ └────────────────┬───────────────┘
                                     │                                │                                  │
                                     └────────────────────────────────┴──────────────────────────────────┘
                                                                      │
                                     ┌────────────────────────────────┴──────────────────────────────────┐
                                     ▼                                                                   ▼
                    ┌────────────────────────────────┐                                  ┌────────────────────────────────┐
        Wave 2      │ 04 doc-and-annotation-cleanup  │                                  │ 05 dedup-scroll-helpers        │
        (sequential)│   M2 + M3 + m4 + m6 + m10      │                                  │   m9                           │
                    └────────────────────────────────┘                                  └────────────────────────────────┘
```

## Background References

| Concern | Path | Notes |
|---|---|---|
| The handler bug | `crates/fdemon-app/src/handler/update.rs:1875-1921` | `VmServicePerformanceMonitoringStarted` handler — C1 lives at line 1920 (alloc unpause guard) |
| Mouse routing bug | `crates/fdemon-app/src/handler/mouse/devtools.rs:97-99` | `DevToolsPanel::Memory` arm delegates to `handle_performance_scroll` |
| Memory disconnected gap | `crates/fdemon-tui/src/widgets/devtools/mod.rs:158-172` | `_vm_connected` captured but discarded for the Memory arm |
| DetailsTab dead variant | `crates/fdemon-app/src/session/performance.rs:16-38` | `PerfSection::{FrameChart, DetailsTab}` + `next()`/`prev()` |
| DetailsTab no-op handlers | `crates/fdemon-app/src/handler/devtools/performance.rs:111, 150, 180, 203` | Four match arms that silently no-op when `focused_section == DetailsTab` |
| `format_number` regression | `crates/fdemon-tui/src/widgets/devtools/memory/mod.rs:54-65` | K/M/G suffix path; called from `table.rs:213` |
| Cell exception registry | `docs/REVIEW_FOCUS.md:29-34` | "Current usage" list — needs two new entries |
| Duplicated helpers | `crates/fdemon-app/src/handler/devtools/performance.rs:31-40` + `.../memory.rs:32-41` | `clamp_chart_scroll` + `ScrollDir` |
| Architecture contract | `docs/ARCHITECTURE.md:908` | Documents the "either Performance or Memory unpauses alloc" invariant |

## Open Decision Handed to T03

**M4 — `PerfSection::DetailsTab` keypress trap.** T03 will pick one of:

- **Option A (YAGNI):** Collapse `PerfSection::next()` to return `FrameChart` when there's no DetailsTab content. Tab becomes a visible no-op. Keep the variant for forward compat or drop it entirely.
- **Option B (visible stub):** Render a "Details (Phase 2)" placeholder pane when focus is on `DetailsTab`. Update Performance footer to advertise Tab. Larger diff, better discoverability.

T03's task file presents both options; the implementor picks and documents the choice in the Completion Summary. Either option satisfies the acceptance criterion (Tab on Performance must not silently break arrow keys).

## Success Criteria

Phase 1-followup is complete when:

- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` is green.
- [ ] Setting `default_panel = "memory"` and opening DevTools shows live memory data within 2 seconds (alloc polling actually runs).
- [ ] Mouse-wheel scrolling on the Memory tab moves the memory chart / alloc table — never the hidden frame chart.
- [ ] Performance tab is no longer stuck on "performance monitoring starting…" — committed regression test enforces this.
- [ ] Memory tab renders a clear "VM not connected" state when the Dart VM is unreachable, mirroring Performance.
- [ ] Tab on the Performance panel either no-ops visibly (Option A) or surfaces a visible Details placeholder (Option B); arrow keys never silently die.
- [ ] Allocation table instance counts display with comma separators ("12,345") not K/M/G suffixes.
- [ ] `docs/REVIEW_FOCUS.md` lists `MemoryState::memory_chart_visible_width` and `MemoryState::alloc_table_visible_height` under "Current usage".
- [ ] Module docstrings on `widgets/devtools/performance/mod.rs` and `handler/devtools/performance.rs` reflect post-T03 reality (no mentions of dual sections or memory handlers).
- [ ] `clamp_chart_scroll` and `ScrollDir` exist in exactly one place under `handler/devtools/`.

## Notes

- The original Phase 1 review concerns map to tasks as follows:

  | Severity | Finding | Task |
  |---|---|---|
  | Critical | C1 — alloc unpause guard | T01 |
  | Critical | C2 — mouse memory scroll routing | T01 |
  | Critical | C3 — `performance.monitoring_active` (fixed in-flight) | T01 (commit it) |
  | Major | M1 — Memory disconnected-state UI | T02 |
  | Major | M2 — stale module docstrings | T04 |
  | Major | M3 — `REVIEW_FOCUS.md` missing Cell entries | T04 |
  | Major | M4 — `DetailsTab` keypress trap | T03 |
  | Major | M5 — `format_number` precision regression | T02 |
  | Minor | m1 — stale `format_number` comment | T02 |
  | Minor | m2 — `#[allow(dead_code)]` rationale | T02 |
  | Minor | m3 — `Layout::vertical` refactor in memory render | T02 |
  | Minor | m4 — EXCEPTION annotation cross-refs | T04 |
  | Minor | m5 — `test_tab_bar_shows_all_panels` expansion | T02 |
  | Minor | m6 — Performance footer keymap drift | T04 |
  | Minor | m7 — missing test for Perf↔Memory switch | T01 |
  | Minor | m8 — missing test for memory-default unpause | T01 |
  | Minor | m9 — `clamp_chart_scroll` duplication | T05 |
  | Minor | m10 — `monitoring_active` co-set invariant doc | T04 |
  | Minor | m11 — `PerfSection::DetailsTab` rename | T03 |
  | Minor | m12 — `_vm_connected` TODO comment | T02 (subsumed by M1) |
