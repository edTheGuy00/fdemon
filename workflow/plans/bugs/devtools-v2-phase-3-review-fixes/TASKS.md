# DevTools v2 Phase 3 Review Fixes - Task Index

## Overview

Fix all issues identified in the Phase 3 Performance Tab review. Two critical bugs, three major structural improvements, and a minor cleanup batch.

**Total Tasks:** 7
**Waves:** 3 (01+02+03 parallel, then 04+05+06 parallel, then 07 solo)

## Task Dependency Graph

```
Wave 1 (parallel — independent fixes in different files)
┌──────────────────────────────┐  ┌──────────────────────────────┐  ┌──────────────────────────────┐
│ 01-fix-alloc-table-layout    │  │ 02-fix-utf8-truncation       │  │ 03-fix-selected-frame-wrap   │
│ (fdemon-tui performance/)    │  │ (3 files, 3 crates)          │  │ (fdemon-app handler/)        │
└──────────────┬───────────────┘  └──────────────┬───────────────┘  └──────────────┬───────────────┘
               │                                  │                                 │
Wave 2 (parallel — structural improvements)       │                                 │
               ▼                                  ▼                                 ▼
┌──────────────────────────────┐  ┌──────────────────────────────┐  ┌──────────────────────────────┐
│ 04-extract-memory-chart-mods │  │ 06-extract-frame-chart-mods  │  │ 05-dedup-frame-nav-logic     │
│ depends: 01, 02              │  │ depends: none                │  │ depends: 03                  │
└──────────────────────────────┘  └──────────────────────────────┘  └──────────────────────────────┘
                                           │
Wave 3 (solo — cleanup)                    │
               ┌───────────────────────────┘
               ▼
┌──────────────────────────────┐
│ 07-minor-fixes-batch         │
│ depends: 04, 05              │
└──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Crate | Modules |
|---|------|--------|------------|-------|---------|
| 1 | [01-fix-alloc-table-layout](tasks/01-fix-alloc-table-layout.md) | Not Started | - | `fdemon-tui` | `performance/mod.rs`, `memory_chart.rs`, `devtools/mod.rs` |
| 2 | [02-fix-utf8-truncation](tasks/02-fix-utf8-truncation.md) | Not Started | - | `fdemon-tui`, `fdemon-app` | `memory_chart.rs`, `search_input.rs`, `session.rs` |
| 3 | [03-fix-selected-frame-wrap](tasks/03-fix-selected-frame-wrap.md) | Not Started | - | `fdemon-app` | `handler/update.rs` |
| 4 | [04-extract-memory-chart-mods](tasks/04-extract-memory-chart-mods.md) | Not Started | 1, 2 | `fdemon-tui` | `memory_chart/` directory |
| 5 | [05-dedup-frame-nav-logic](tasks/05-dedup-frame-nav-logic.md) | Not Started | 3 | `fdemon-app` | `handler/keys.rs`, `session/performance.rs` |
| 6 | [06-extract-frame-chart-mods](tasks/06-extract-frame-chart-mods.md) | Not Started | - | `fdemon-tui` | `frame_chart/` directory |
| 7 | [07-minor-fixes-batch](tasks/07-minor-fixes-batch.md) | Not Started | 4, 5 | workspace | Multiple files |

## Dispatch Plan

**Wave 1** (parallel — independent critical/major bug fixes):
- Task 01: Fix allocation table layout threshold (fdemon-tui performance layout)
- Task 02: Fix UTF-8 byte-slice panics (3 files across 2 crates)
- Task 03: Fix selected_frame stale index on ring buffer wrap (fdemon-app handler)

**Wave 2** (parallel — structural improvements after bug fixes land):
- Task 04: Extract memory_chart.rs into submodules (depends on 01+02 landing in same file)
- Task 05: Deduplicate frame navigation logic (depends on 03 touching related handler code)
- Task 06: Extract frame_chart.rs into submodules (independent)

**Wave 3** (solo — final cleanup):
- Task 07: Minor fixes batch (depends on 04+05 to avoid merge conflicts)

## Success Criteria

All fixes are complete when:

- [ ] Allocation table visible with at least 2 data rows on 24-row terminal
- [ ] No UTF-8 panics on multi-byte character truncation (3 sites fixed + tests)
- [ ] `selected_frame` tracks same logical frame through ring buffer wraps
- [ ] Frame navigation logic has single source of truth
- [ ] `memory_chart.rs` under 500 lines (currently 711)
- [ ] `frame_chart.rs` under 500 lines (currently 544)
- [ ] All minor issues resolved (duplicate test, anti-pattern, dead state, magic number, visibility)
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings`

## Notes

- **Wave 2 ordering**: Tasks 04 and 06 (file extractions) are large refactors that restructure files touched by Wave 1 fixes. They must wait for those fixes to land to avoid merge conflicts.
- **Task 06 has no blockers** but is grouped in Wave 2 because it's a structural improvement, not a bug fix.
- **Task 07 is intentionally last** — several minor items touch the same files as earlier tasks.
