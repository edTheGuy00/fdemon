# Phase 1-Followup — Review-Driven Fixes — Task Index

## Overview

Five tasks address the 3 Critical + 5 Major + 12 Minor findings from the Phase 1 code review ([`../../../../reviews/features/devtools-performance-memory-split-phase-1/REVIEW.md`](../../../../reviews/features/devtools-performance-memory-split-phase-1/REVIEW.md)). See [`PLAN.md`](PLAN.md) for the rationale, task↔finding mapping, and the open Option A/B decision for T03.

- **Wave 1 (parallel):** Three independent fix tracks — critical handler bugs (T01), memory panel UX/format (T02), and the `DetailsTab` trap + rename (T03). Zero write-file overlap.
- **Wave 2 (sequential):** Doc/annotation cleanup (T04) and scroll-helper deduplication (T05). Both touch files written in Wave 1 and one another.

**Total Tasks:** 5
**Estimated Hours:** 6–9 hours

## Task Dependency Graph

```
        ┌────────────────────────────────┐   ┌──────────────────────────────┐   ┌────────────────────────────────┐
Wave 1  │ 01 fix-critical-handler-bugs   │   │ 02 memory-panel-followups    │   │ 03 resolve-details-tab-trap    │
        │  (handler/*; C1+C2+C3 commit)  │   │  (memory widget tree)        │   │  (perf widget tree + rename)   │
        └──────────────┬─────────────────┘   └──────────────┬───────────────┘   └────────────────┬───────────────┘
                       │                                    │                                    │
                       └────────────────────────────────────┴────────────────────────────────────┘
                                                            │
                                       ┌────────────────────┴────────────────────┐
                                       ▼                                         ▼
        ┌────────────────────────────────┐                            ┌────────────────────────────────┐
Wave 2  │ 04 doc-and-annotation-cleanup  │ ──── must precede ─────►   │ 05 dedup-scroll-helpers        │
        │  (REVIEW_FOCUS, docstrings,    │   (overlap on handler/     │  (extract clamp_chart_scroll)  │
        │   annotations, footer)         │    devtools/performance)   │                                │
        └────────────────────────────────┘                            └────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Wave |
|---|------|--------|------------|------------|-------|------|
| 01 | [fix-critical-handler-bugs](tasks/01-fix-critical-handler-bugs.md) | Not Started | — | 2–3h | implementor | 1 |
| 02 | [memory-panel-followups](tasks/02-memory-panel-followups.md) | Not Started | — | 2–3h | implementor | 1 |
| 03 | [resolve-details-tab-trap](tasks/03-resolve-details-tab-trap.md) | Not Started | — | 1.5–2h | implementor | 1 |
| 04 | [doc-and-annotation-cleanup](tasks/04-doc-and-annotation-cleanup.md) | Not Started | 02, 03 | 1h | implementor | 2 |
| 05 | [dedup-scroll-helpers](tasks/05-dedup-scroll-helpers.md) | Not Started | 03, 04 | 0.5–1h | implementor | 2 |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|-------------------------|---------------------------|
| **01** fix-critical-handler-bugs | `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/mouse/devtools.rs`, `crates/fdemon-app/src/handler/tests.rs` | `crates/fdemon-app/src/handler/devtools/memory.rs` (handler signatures for `Mem*` messages), `crates/fdemon-app/src/message.rs` (variant names), `crates/fdemon-app/src/state.rs` (`DevToolsPanel::Memory`) |
| **02** memory-panel-followups | `crates/fdemon-tui/src/widgets/devtools/mod.rs` (Memory dispatch arm + `test_tab_bar_shows_all_panels`), `crates/fdemon-tui/src/widgets/devtools/memory/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/memory/table.rs`, `crates/fdemon-tui/src/widgets/devtools/memory/tests.rs` | `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` (`render_disconnected` pattern), `crates/fdemon-app/src/state.rs` (`ConnectionStatus`) |
| **03** resolve-details-tab-trap | `crates/fdemon-app/src/session/performance.rs`, `crates/fdemon-app/src/handler/devtools/performance.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` | `crates/fdemon-app/src/handler/devtools/inspector/` (DetailsTab name reference to avoid) |
| **04** doc-and-annotation-cleanup | `docs/REVIEW_FOCUS.md`, `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` (module docstring only), `crates/fdemon-app/src/handler/devtools/performance.rs` (module docstring only), `crates/fdemon-app/src/session/memory.rs` (EXCEPTION annotation lines), `crates/fdemon-app/src/session/performance.rs` (EXCEPTION annotation + invariant comment), `crates/fdemon-tui/src/widgets/devtools/mod.rs` (Performance footer hint string only) | T02 + T03 completion summaries |
| **05** dedup-scroll-helpers | `crates/fdemon-app/src/handler/devtools/mod.rs` (or NEW `crates/fdemon-app/src/handler/devtools/scroll_helpers.rs`), `crates/fdemon-app/src/handler/devtools/performance.rs`, `crates/fdemon-app/src/handler/devtools/memory.rs` | — |

### Overlap Matrix (write-files only)

| Pair | Shared Write Files | Wave | Strategy |
|------|--------------------|------|----------|
| 01 + 02 | **None** | 1 | **Parallel (worktree)** — T01 lives entirely under `crates/fdemon-app/src/handler/{update,mouse,tests}`; T02 lives entirely under `crates/fdemon-tui/src/widgets/devtools/memory/` plus the Memory dispatch arm + test in `widgets/devtools/mod.rs`. Zero intersection. |
| 01 + 03 | **None** | 1 | **Parallel (worktree)** — T01 touches `handler/{update,mouse,tests}`; T03 touches `session/performance.rs`, `handler/devtools/performance.rs`, and `widgets/devtools/performance/`. Distinct file trees. |
| 02 + 03 | **None** | 1 | **Parallel (worktree)** — T02 is memory-side; T03 is performance-side. The shared parent module `widgets/devtools/mod.rs` is only edited by T02 (Memory dispatch + tab-bar test). T03 does **not** touch `widgets/devtools/mod.rs`. |
| 04 + 02 | `crates/fdemon-tui/src/widgets/devtools/mod.rs` (T02: Memory dispatch + test; T04: Performance footer hint string) | — | **Sequential** — T04 runs after T02 merges. The edits are non-overlapping lines but in the same file. |
| 04 + 03 | `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`, `crates/fdemon-app/src/handler/devtools/performance.rs`, `crates/fdemon-app/src/session/performance.rs` | — | **Sequential** — T04 runs after T03 merges. T04 only touches the module-level `//!` docstrings on the two `.rs` files plus the EXCEPTION annotation/invariant comment lines on `session/performance.rs`; T03 has already settled the structural changes. |
| 05 + 03 | `crates/fdemon-app/src/handler/devtools/performance.rs` | — | **Sequential** — T05 dedups `clamp_chart_scroll` after T03 finalises the handler bodies. |
| 04 + 05 | `crates/fdemon-app/src/handler/devtools/performance.rs` (T04: module docstring; T05: helper extraction + import) | 2 | **Sequential within Wave 2** — T04 first (docstring is independent of body), then T05 (dedup may move surrounding lines). |

## Success Criteria

Phase 1-followup is complete when:

- [ ] Full quality gate green: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] **C1 verified:** `default_panel = "memory"` cold start → `alloc_pause_tx` is sent `false`. Regression test `test_lazy_start_memory_default_unpauses_alloc` passes.
- [ ] **C2 verified:** Mouse-wheel on Memory tab dispatches `Mem*` messages, not `Perf*`. `session.performance.frame_chart_scroll_offset` is unchanged after Memory wheel events.
- [ ] **C3 verified:** `performance.monitoring_active == true` after `Message::VmServicePerformanceMonitoringStarted`. Regression test asserts both flags flip.
- [ ] **M1 verified:** Memory panel renders a disconnected-state view when `vm_connected == false`, mirroring Performance.
- [ ] **M2 verified:** Module docstrings on `widgets/devtools/performance/mod.rs` and `handler/devtools/performance.rs` contain no references to dual-section layout, memory chart, or allocation profile handling.
- [ ] **M3 verified:** `docs/REVIEW_FOCUS.md` "Current usage" section lists `MemoryState::memory_chart_visible_width` and `MemoryState::alloc_table_visible_height`.
- [ ] **M4 verified:** Pressing Tab on the Performance panel does not silently break arrow keys (Option A: Tab no-ops; Option B: visible Details placeholder).
- [ ] **M5 verified:** Allocation table renders instance counts with comma separators (e.g. "12,345") for all values ≥ 1,000.
- [ ] **m9 verified:** Exactly one definition of `clamp_chart_scroll` and `ScrollDir` exists in `crates/fdemon-app/src/handler/devtools/`.
- [ ] All other minor items (m1–m12 except m9 covered above) addressed within their bundling task.

## Phase Acceptance Test Plan

After all 5 tasks merge, run the manual smoke sequence:

1. `cargo run -- ~/Dev/some-flutter-app` in a 200×20 terminal split.
2. Press `d` → DevTools, press `m` → Memory tab. Verify the memory chart + alloc table render with live data within 2 seconds.
3. Mouse-scroll wheel on the alloc table. Verify rows scroll (not the hidden frame chart).
4. Set `default_panel = "memory"` in `.fdemon/config.toml`, restart, press `d`. Verify Memory tab shows live data without manually switching tabs.
5. Press `p`, press Tab. Verify behaviour matches T03's chosen option — either nothing visibly changes (Option A) or a "Details" placeholder appears (Option B). Press `j/k`. Verify arrow keys work whichever option was chosen.
6. Inspect an allocation row with 12,345 instances. Verify it renders as "12,345", not "12.3K".
7. Disconnect the Dart VM (kill `flutter run`). Verify the Memory tab shows a clear disconnected-state message matching Performance.

## Notes

- This phase blocks the start of original PLAN.md Phase 2 (Performance details expansion). Phase 2 should rebase on the post-followup branch.
- No new VM Service work. No changes under `crates/fdemon-core/` or `crates/fdemon-daemon/`.
- `docs/KEYBINDINGS.md` is unchanged — Phase 1's T04 already documented the post-split keymap correctly. T04 of this phase only touches the in-app *footer hint string* in `widgets/devtools/mod.rs`, not the user-facing keyboard doc.
- The in-flight fix for C3 (in `update.rs` and `handler/tests.rs`) lives in the working tree at the time T01 starts. T01 must either fold it into a new commit alongside C1 + C2, or simply commit it first and add C1/C2 on top.
