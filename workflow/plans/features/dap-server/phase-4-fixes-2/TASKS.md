# DAP Adapter mod.rs Split — Task Index

## Overview

The `crates/fdemon-dap/src/adapter/mod.rs` file is 8,025 lines (2,846 production + 5,178 tests). Task 02 from phase-4-fixes created split files (`handlers.rs`, `events.rs`, `variables.rs`, `types.rs`, `backend.rs`) but never wired them up — no `mod` declarations were added and the original code was never removed. Subsequent tasks (03–15) continued modifying `mod.rs` directly, making the dead files stale and the live file larger.

**Goal:** Split `mod.rs` down to ~200 lines of production code (struct + constructors + re-exports) and ~0 inline tests. CODE_STANDARDS.md mandates files > 500 lines be split into submodules.

**Total Tasks:** 6
**Waves:** 6 (all sequential — single-file extraction requires ordered edits)

## Current State

| Section | Lines | Target File |
|---------|-------|-------------|
| Imports, struct, constructors, re-exports | ~120 | `mod.rs` (keep) |
| `DebugBackend` + `DynDebugBackend` | ~420 | `backend.rs` |
| Types, enums, constants, helpers | ~250 | `types.rs` |
| Event handling (`handle_debug_event`, etc.) | ~530 | `events.rs` |
| Request handlers (`handle_request`, etc.) | ~670 | `handlers.rs` |
| Variable/scope handling | ~620 | `variables.rs` |
| Tests (163 functions, 14 mock backends) | ~5,178 | `tests/` submodules |

## Task Dependency Graph

```
Wave 1 — Foundation
└── 01-delete-stale-extract-types-backend

Wave 2
└── 02-extract-events           (depends on 01)

Wave 3
└── 03-extract-handlers         (depends on 02)

Wave 4
└── 04-extract-variables        (depends on 03)

Wave 5
└── 05-move-mocks-to-helpers    (depends on 04)

Wave 6
└── 06-split-test-module        (depends on 05)
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-delete-stale-extract-types-backend](tasks/01-delete-stale-extract-types-backend.md) | Done | - | `adapter/mod.rs`, `adapter/types.rs`, `adapter/backend.rs` |
| 2 | [02-extract-events](tasks/02-extract-events.md) | Done | 01 | `adapter/mod.rs`, `adapter/events.rs` |
| 3 | [03-extract-handlers](tasks/03-extract-handlers.md) | Done | 02 | `adapter/mod.rs`, `adapter/handlers.rs` |
| 4 | [04-extract-variables](tasks/04-extract-variables.md) | Done | 03 | `adapter/mod.rs`, `adapter/variables.rs` |
| 5 | [05-move-mocks-to-helpers](tasks/05-move-mocks-to-helpers.md) | Done | 04 | `adapter/mod.rs`, `adapter/test_helpers.rs` |
| 6 | [06-split-test-module](tasks/06-split-test-module.md) | Done | 05 | `adapter/mod.rs`, `adapter/tests/*.rs` |

## Success Criteria

- [ ] `adapter/mod.rs` production code ≤ 300 lines (struct + constructors + re-exports)
- [ ] `adapter/mod.rs` total (including inline tests) ≤ 500 lines
- [ ] No file in `adapter/` exceeds 800 lines (excluding test modules)
- [ ] No dead/stale files on disk
- [ ] `cargo fmt --all` — Pass
- [ ] `cargo check --workspace` — Pass
- [ ] `cargo test --workspace` — Pass (all existing tests green)
- [ ] `cargo clippy --workspace -- -D warnings` — Pass

## Notes

- All tasks are sequential because each one modifies `mod.rs` (removing a section)
- The dead files (`handlers.rs`, `events.rs`, `variables.rs`, `types.rs`, `backend.rs`) are stale — they lack fixes from tasks 03–15 and must be deleted, not reused
- The `test_helpers.rs` module already exists and contains `MockTestBackend` trait — task 05 extends it with the 10 concrete mock backends from the test module
- Task 06 (test split) is the largest — ~5,000 lines of tests split into ~8 themed files
