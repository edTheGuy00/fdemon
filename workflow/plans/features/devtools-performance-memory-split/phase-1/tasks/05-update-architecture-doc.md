## Task: Update `docs/ARCHITECTURE.md` — Document the DevTools Performance / Memory Split

**Agent:** doc_maintainer

**Objective**: Update the DevTools subsystem section of `docs/ARCHITECTURE.md` to reflect the Phase 1 tab split: add `DevToolsPanel::Memory`, document the new `MemoryState` per-session struct, the new `handler/devtools/memory.rs` module, the new `widgets/devtools/memory/` widget subtree, and the renamed `MemoryChart` → `MemoryPanel` widget. Slim references to the old monolithic Performance state where they still appear.

**Depends on**: 03-extract-memory-handlers-and-widgets

**Estimated Time**: 1–1.5 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — Content boundary rules.
- `workflow/plans/features/devtools-performance-memory-split/PLAN.md` — Phase 1 §6.
- `workflow/plans/features/devtools-performance-memory-split/phase-1/tasks/01-add-memory-panel-placeholder.md`
- `workflow/plans/features/devtools-performance-memory-split/phase-1/tasks/02-extract-memory-state.md`
- `workflow/plans/features/devtools-performance-memory-split/phase-1/tasks/03-extract-memory-handlers-and-widgets.md`
- `crates/fdemon-app/src/session/memory.rs` (the final implemented state) — for accuracy of field names and module references.

### Change Context

1. **New `DevToolsPanel::Memory` variant** — the DevTools panel enum gains a fourth tab, in tab-bar position 3 (between Performance and Network). The default tab remains Inspector.
2. **`PerformanceState` slim-down** — memory-related fields (memory_history, gc_history, memory_samples, allocation_profile, allocation_sort, alloc_table_*, memory_chart_*) moved to a sibling `MemoryState` struct on `Session`. `PerfSection` shrank from 3 variants to 2 (`FrameChart`, `DetailsTab`).
3. **New `MemoryState` struct in `crates/fdemon-app/src/session/memory.rs`** — per-session memory monitoring state with its own `MemorySection { Chart, AllocationList }` enum, default constants (`DEFAULT_MEMORY_HISTORY_SIZE = 60`, `DEFAULT_GC_HISTORY_SIZE = 50`, `DEFAULT_MEMORY_SAMPLE_SIZE = 120`), and `AllocationSortColumn` (relocated from `performance.rs`).
4. **New `handler/devtools/memory.rs`** — handler module containing `handle_memory_sample_received`, `handle_allocation_profile_received`, `handle_toggle_allocation_sort`, `handle_mem_focus_section`, `handle_mem_scroll`, `handle_mem_page`, `handle_mem_jump_to_start`, `handle_mem_jump_to_end`, `handle_mem_select_alloc_row`. Mirrors the layout of `handler/devtools/performance.rs`.
5. **New `widgets/devtools/memory/` widget subtree** — five files (`mod.rs`, `chart.rs`, `table.rs`, `braille_canvas.rs`, `tests.rs`) moved from `widgets/devtools/performance/memory_chart/`. Top-level widget renamed `MemoryChart` → `MemoryPanel`. The directory `widgets/devtools/performance/memory_chart/` no longer exists.
6. **New `Mem*` `Message` variants** — `MemFocusSection(MemorySection)`, `MemScrollUp/Down`, `MemPageUp/Down`, `MemJumpToStart/End`, `MemSelectAllocRow`, `MemToggleSort`. `ToggleAllocationSort` and `PerfSelectAllocRow` variants renamed to their `Mem*` equivalents. `PerfFocusSection(PerfSection)` payload type narrowed.
7. **`alloc_pause_tx` shared between Performance and Memory tabs** — entering either tab unpauses allocation profile polling. The `handle_switch_panel` arms for both tabs run the same `tx.send(false)` side-effect.

### Sections of ARCHITECTURE.md to Update

The exact section names depend on the current document structure. Target updates:

- **Workspace Crates / `fdemon-app` description** — update the bullet that lists `session/` subfiles to include `memory.rs` (e.g., "DevTools handlers in `handler/devtools/` with per-session state (`PerformanceState`, `NetworkState`, **`MemoryState`**)").
- **Workspace Crates / `fdemon-tui` description** — update the bullet that lists DevTools panels to include `widgets/devtools/memory/` and `MemoryPanel`. Note that the `performance/memory_chart/` subdirectory has been removed.
- **DevTools Subsystem section (if it exists)** — add `DevToolsPanel::Memory` to the enumeration of panels; document the responsibilities and which state struct each panel reads.
- **Per-session DevTools state subsection** — add `MemoryState` alongside `PerformanceState` and `NetworkState`; describe what it holds (memory history, GC events, allocation profile, alloc table scroll/selection state).
- **DevTools handler decomposition subsection** — list `handler/devtools/memory.rs` as a new sibling of `inspector.rs`, `performance.rs`, `network.rs`.
- **Key Patterns / Service Layer / VM Service Client** — no changes (Phase 1 does not touch the daemon or VM Service).
- **Per-tab keymap** — if the document has a section on DevTools keymap or input dispatching, mention the new `in_memory` guard alongside `in_performance` / `in_network` / `in_inspector`.

### Content Boundary Reminders

- **ARCHITECTURE.md only describes structure and data flow.** Do NOT include:
  - Build commands or test commands (those belong in `DEVELOPMENT.md`).
  - Coding conventions like "use named constants" or "no magic numbers" (those belong in `CODE_STANDARDS.md`).
  - Specific keyboard shortcuts (those belong in `KEYBINDINGS.md`, updated in T04).
- **DO** include:
  - Where files live and what they own.
  - How data flows between modules (e.g., "VM Service emits `VmServiceMemorySnapshot` → `update.rs` writes to `session.memory.memory_history` → widget reads from `&session.memory`").
  - Layer dependencies (e.g., "`widgets/devtools/memory/` depends on `fdemon-core` and `fdemon-app`; it does NOT depend on `fdemon-daemon`").
  - The TEA exception status of the new render-hint Cells on `MemoryState` (which are the same pattern as the existing Cell render-hints — they don't introduce a new exception class).

### Acceptance Criteria

1. ARCHITECTURE.md references `DevToolsPanel::Memory` and lists it in the enumeration of DevTools panels.
2. ARCHITECTURE.md references `MemoryState` and `MemorySection` (state) and `handler/devtools/memory.rs` (handler module) and `widgets/devtools/memory/` (widget subtree).
3. ARCHITECTURE.md no longer describes `PerformanceState` as holding memory data (memory history, GC events, allocation profile).
4. ARCHITECTURE.md no longer references `widgets/devtools/performance/memory_chart/` (the removed subdirectory).
5. No new architectural pattern is described — Phase 1 is a refactor; the patterns are already documented (TEA, EngineEvent, mouse-region registry, render-hint Cell). Verify the existing TEA exception list in REVIEW_FOCUS.md still applies (`MemoryState.memory_chart_visible_width` and `MemoryState.alloc_table_visible_height` are the renamed cells — same exception class, no new entry needed).
6. No content boundary violations — all bullet additions are structural (file paths, data flow), not standards or commands.
7. `doc-standards` validation (`/doc-validate`) passes after the edit.

### Notes

- Make **targeted edits**. Do not rewrite the document — find the sections that mention `PerformanceState` / `widgets/devtools/performance/memory_chart` / `DevToolsPanel::{Inspector, Performance, Network}` and update those lines.
- If a hub-and-spoke variant exists (`docs/devtools_ARCHITECTURE.md` or similar), apply the same edits there with consistent terminology.
- Cross-reference: if ARCHITECTURE.md links to KEYBINDINGS.md for the DevTools keymap, that link does not need to change — T04 updates the linked document.
- The naming `MemoryPanel` (widget) vs `MemoryState` (state) vs `DevToolsPanel::Memory` (enum variant) is consistent with the existing `WidgetInspector` / `InspectorState` / `DevToolsPanel::Inspector` triple — point this out if a "naming conventions" section exists.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Added `DevToolsPanel::Memory` to panel enumeration and `DevToolsViewState` description; added `MemoryState` and `MemorySection` documentation in Panel State Model and new Memory Panel Interactivity section; added `handler/devtools/memory.rs` to project tree and DevTools handler diagram; added `widgets/devtools/memory/` subtree to project structure; removed `performance/memory_chart/` from tree; updated `performance/` to frame-chart-only; updated `PerfSection` to 2 variants; added `alloc_pause_tx` to `SessionHandle` schema; updated session/ module reference table; updated module listing for `session/memory.rs`. |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: YES/NO/N/A — N/A (no cross-contamination)

### Notable Decisions/Tradeoffs

1. **Memory Panel Interactivity section is new**: Added alongside the existing "Performance Panel Interactivity" section to document `MemoryState`, `MemorySection`, render-hint cells, buffer size constants, and `AllocationSortColumn`. This is structural/data-flow documentation (not coding conventions), so it belongs in ARCHITECTURE.md.
2. **`AllocationSortColumn` relocation noted**: The doc notes that this enum moved from `session/performance.rs` to `session/memory.rs` as part of Phase 1. This is factual structural context, not a style guideline.
3. **No new TEA exception class**: Explicitly noted that `MemoryState.memory_chart_visible_width` and `MemoryState.alloc_table_visible_height` are renames of fields that previously lived on `PerformanceState` — same approved exception class, no new entry required.

### Testing Performed

- All acceptance criteria verified via grep checks
- No content boundary violations detected
