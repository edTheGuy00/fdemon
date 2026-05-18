# Phase 1 — Tab Split (Performance / Memory) — Task Index

## Overview

Phase 1 splits the current monolithic Performance DevTools tab into two top-level tabs: **Performance** (frame timing only) and **Memory** (memory chart + allocation table). This is a pure refactor — no new features, no new RPCs. Each tab gets the full panel inner area after the split, eliminating the layout bugs that hide frame timing and crop the allocation list on short-wide terminals.

The refactor crosses two crates (`fdemon-app`, `fdemon-tui`) and three concerns (state, handlers, widgets). To minimise merge conflicts in worktree-parallel execution we run the work in three waves:

- **Wave 1 (parallel):** Two independent surfaces — a foundation task that adds the `Memory` tab placeholder, and a data-layer task that extracts `MemoryState`. They share zero write-files.
- **Wave 2 (sequential):** Handler extraction + widget move + Memory tab dispatch — this is the single task that turns the placeholder into the real Memory panel and depends on both Wave 1 outputs.
- **Wave 3 (parallel):** Two doc updates.

**Total Tasks:** 5
**Estimated Hours:** 14–20 hours

## Task Dependency Graph

```
        ┌──────────────────────────────────┐   ┌──────────────────────────────────┐
Wave 1  │ 01-add-memory-panel-placeholder  │   │ 02-extract-memory-state          │
        │  (enum, tab bar, 'm', dispatch)  │   │  (MemoryState, write redirect)   │
        └──────────────────────┬───────────┘   └──────────────────┬───────────────┘
                               │                                  │
                               └───────────────┬──────────────────┘
                                               ▼
        ┌────────────────────────────────────────────────────────────────────────┐
Wave 2  │ 03-extract-memory-handlers-and-widgets                                 │
        │  (memory.rs handler, Mem* messages, in_memory keymap, widget move)     │
        └─────────────────────────────────────┬──────────────────────────────────┘
                               ┌──────────────┴───────────────┐
                               ▼                              ▼
        ┌──────────────────────────────────┐   ┌──────────────────────────────────┐
Wave 3  │ 04-update-keybindings-doc        │   │ 05-update-architecture-doc       │
        │  (KEYBINDINGS.md)                │   │  (ARCHITECTURE.md, doc_maint)    │
        └──────────────────────────────────┘   └──────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Wave |
|---|------|--------|------------|------------|-------|------|
| 01 | [add-memory-panel-placeholder](tasks/01-add-memory-panel-placeholder.md) | Not Started | — | 2–3h | implementor | 1 |
| 02 | [extract-memory-state](tasks/02-extract-memory-state.md) | Not Started | — | 4–6h | implementor | 1 |
| 03 | [extract-memory-handlers-and-widgets](tasks/03-extract-memory-handlers-and-widgets.md) | Not Started | 01, 02 | 6–8h | implementor | 2 |
| 04 | [update-keybindings-doc](tasks/04-update-keybindings-doc.md) | Not Started | 03 | 0.5h | implementor | 3 |
| 05 | [update-architecture-doc](tasks/05-update-architecture-doc.md) | Not Started | 03 | 1–1.5h | doc_maintainer | 3 |

## File Overlap Analysis

> The orchestrator uses this section to decide isolation strategy per wave. Read-only overlap is fine — only **write-file** overlap forces sequential execution.

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| **01** add-memory-panel-placeholder | `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/handler/devtools/mod.rs`, `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-tui/src/widgets/devtools/mod.rs` | (existing structure of `widgets/devtools/performance/mod.rs`, `widgets/devtools/inspector/mod.rs`) |
| **02** extract-memory-state | `crates/fdemon-app/src/session/memory.rs` (NEW), `crates/fdemon-app/src/session/performance.rs`, `crates/fdemon-app/src/session/session.rs`, `crates/fdemon-app/src/session/mod.rs`, `crates/fdemon-app/src/update.rs`, `crates/fdemon-app/src/handler/devtools/performance.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/tests.rs` | `crates/fdemon-core/src/performance.rs` (MemoryUsage, GcEvent, MemorySample, AllocationProfile types) |
| **03** extract-memory-handlers-and-widgets | `crates/fdemon-app/src/handler/devtools/memory.rs` (NEW), `crates/fdemon-app/src/handler/devtools/performance.rs`, `crates/fdemon-app/src/handler/devtools/mod.rs`, `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-app/src/message.rs`, `crates/fdemon-tui/src/widgets/devtools/memory/mod.rs` (NEW — moved from `performance/memory_chart/mod.rs`), `crates/fdemon-tui/src/widgets/devtools/memory/chart.rs` (NEW — moved), `crates/fdemon-tui/src/widgets/devtools/memory/table.rs` (NEW — moved), `crates/fdemon-tui/src/widgets/devtools/memory/braille_canvas.rs` (NEW — moved), `crates/fdemon-tui/src/widgets/devtools/memory/tests.rs` (NEW — moved + added migrated tests), `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs`, `crates/fdemon-tui/src/widgets/devtools/mod.rs` (delete old `performance/memory_chart/` directory entirely) | T01 and T02 must be merged into the base branch first |
| **04** update-keybindings-doc | `docs/KEYBINDINGS.md` | T03 task spec |
| **05** update-architecture-doc | `docs/ARCHITECTURE.md` | T01–T03 task specs and completion summaries |

### Overlap Matrix (write-files only)

| Pair | Shared Write Files | Wave | Strategy |
|------|--------------------|------|----------|
| 01 + 02 | **None** | 1 | **Parallel (worktree)** — T01 writes to `state.rs` / `handler/devtools/mod.rs` / `handler/keys.rs` / `widgets/devtools/mod.rs`; T02 writes to `session/*`, `update.rs`, `handler/devtools/performance.rs`, `widgets/devtools/performance/mod.rs`, and the `memory_chart/` subtree. Zero intersection. |
| 01 + 03 | `handler/devtools/mod.rs`, `handler/keys.rs`, `widgets/devtools/mod.rs` | — | **Sequential** — T03 depends on T01 (must run after T01 merges). |
| 02 + 03 | `handler/devtools/performance.rs`, `widgets/devtools/performance/mod.rs`, `widgets/devtools/performance/memory_chart/*` (T03 deletes the subtree T02 was reading) | — | **Sequential** — T03 depends on T02 (must run after T02 merges). |
| 04 + 05 | **None** | 3 | **Parallel** — T04 writes `KEYBINDINGS.md`, T05 writes `ARCHITECTURE.md`. |

## Success Criteria

Phase 1 is complete when:

- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` is green.
- [ ] Switching to the Performance tab in a 200×20 terminal shows the **full Frame Chart visible at all times** (no longer hidden below a memory section).
- [ ] Switching to the Memory tab in a 200×20 terminal shows the **allocation table with 15+ visible rows** (no longer capped at 2–3).
- [ ] Pressing `m` while in DevTools mode switches to the Memory panel.
- [ ] All previous keyboard bindings work: `Tab`/`Shift+Tab` cycles sections within each panel (FrameChart↔DetailsTab on Performance; Chart↔AllocationList on Memory); `j`/`k`/`↑`/`↓` scrolls the focused section; `PageUp/PageDown`/`Home/End` page-jump; `s` toggles allocation sort (now under `in_memory` guard).
- [ ] `Esc` on Memory tab with a selected alloc row deselects first, then exits to Logs on a second press — mirrors Performance frame deselection.
- [ ] Memory polling (`alloc_pause_tx`) is unpaused both when the user enters the Performance tab AND the Memory tab; the underlying VM Service poll continues seamlessly across panel switches.
- [ ] All previous tests pass; new tests added: tab bar 4-region click test, Memory panel `render_with_regions` parity test, in_memory keymap routing tests.
- [ ] `docs/KEYBINDINGS.md` documents the new Memory tab keymap.
- [ ] `docs/ARCHITECTURE.md` documents the new `DevToolsPanel::Memory`, `MemoryState`, `handler/devtools/memory.rs`, and `widgets/devtools/memory/` module.

## Phase-Wide Acceptance Test Plan

After all 5 tasks merge, run the manual smoke test in `tmp/manual-smoke.txt` (create during T03 review) or follow these steps:

1. `cargo run --workspace -- ~/Dev/some-flutter-app` in a 200×20 iTerm split.
2. Press `d` to enter DevTools.
3. Press `p` — verify FrameChart fills the panel; press `←/→` to select frames; verify selection works.
4. Press `m` — verify MemoryPanel fills the panel; verify allocation table has 15+ visible rows; press `Tab` to focus the table; press `j/k` to scroll rows; press `s` to toggle sort.
5. Press `Esc` (with a row selected) — verify the row deselects; press `Esc` again — verify exit to Logs.
6. Re-enter DevTools (`d`), press `m`, press `Esc` — verify direct exit to Logs (no row selected).
7. Press `m`, press `Tab`, press `Esc` (with alloc row selected from previous session if reentered) — verify deselect ordering.

## Keyboard Shortcuts Affected by Phase 1

| Key | Before Phase 1 | After Phase 1 |
|-----|----------------|---------------|
| `m` (DevTools) | Unbound | **NEW: Switch to Memory panel** |
| `Tab`/`Shift+Tab` (Performance) | Cycle `{FrameChart, MemoryChart, MemoryList}` | Cycle `{FrameChart, DetailsTab}` (DetailsTab is a Phase 2 placeholder — Phase 1 keeps it a no-op) |
| `Tab`/`Shift+Tab` (Memory) | (panel did not exist) | **NEW: Cycle `{Chart, AllocationList}`** |
| `s` (Performance) | Toggle alloc sort | **Moved: now under `in_memory` guard, dead on Performance** |
| `s` (Memory) | (panel did not exist) | **NEW: Toggle alloc sort** |
| `Esc` (Performance, frame selected) | Deselect frame | Deselect frame (unchanged) |
| `Esc` (Memory, alloc row selected) | (panel did not exist) | **NEW: Deselect row** |

## Notes

- **`PerfSection` shrinks** from `{FrameChart, MemoryChart, MemoryList}` to `{FrameChart, DetailsTab}`. The `DetailsTab` variant is a Phase 2 anchor — in Phase 1 it exists but renders an empty pane. Cycling Tab between FrameChart and DetailsTab is a visible no-op in Phase 1, but reserving the variant now avoids a second enum migration when Phase 2 lands.
- **`alloc_pause_tx` is shared** — both Performance and Memory tabs unpause it on entry. This avoids stopping allocation profile polling when the user toggles between the two related panels.
- **`AllocationSortColumn` moves** from `session/performance.rs` to `session/memory.rs`. The enum and its `Default` impl come along; no behaviour changes.
- **`PerfFocusSection(PerfSection)` payload changes**: the variant remains in `message.rs` but the embedded enum loses two variants. New `MemFocusSection(MemorySection)` is added in T03.
- **`crates/fdemon-app/src/update.rs`** is over 4000 lines and has two inline blocks that write to `session.performance.memory_history` / `gc_history` — these are the highest-risk migration sites because they live outside the handlers. T02 has explicit checklist items for both.
- **VM Service polling logic** in `crates/fdemon-daemon/src/vm_service/` is **not touched in Phase 1.** The daemon still emits the same `Vm Service*` messages; only the app-side dispatch changes.
- **Memory tests** in `widgets/devtools/performance/memory_chart/tests.rs` (1192 lines, 36 tests) move wholesale to `widgets/devtools/memory/tests.rs` with `use super::*` adjusted. Three obsolete tests in `widgets/devtools/performance/tests.rs` (`test_performance_panel_renders_two_sections`, `test_performance_panel_dual_section_at_min_height`, `test_footer_does_not_overlap_memory_border`) are deleted because the dual-section path is gone. Six memory-related tests migrate to `widgets/devtools/memory/tests.rs`.
- **No new VM Service work** in Phase 1. No changes under `crates/fdemon-core/` or `crates/fdemon-daemon/`.
- **`docs/REVIEW_FOCUS.md`** is unchanged — no new TEA exceptions; the existing `Cell<usize>` render-hint pattern already applies to the moved widgets.
