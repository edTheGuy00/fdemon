# DAP Server Phase 6 Review Fixes — Task Index

## Overview

Address all findings from the Phase 6 code review (`workflow/reviews/features/dap-server-phase-6/REVIEW.md`). Fixes are grouped by file to maximize parallelism — tasks touching different files run concurrently in worktrees.

**Total Tasks:** 11
**Estimated Hours:** 12–18 hours

## Task Dependency Graph

```
Wave 1 (Critical blocking fixes — parallel, no shared files)
┌──────────────────────────────┐  ┌──────────────────────────────┐
│ 01-handlers-critical-fixes   │  │ 02-evaluate-injection-fix    │
│ H1, M5, M7 — handlers.rs    │  │ H2 — evaluate.rs             │
└──────────────┬───────────────┘  └──────────────────────────────┘
               │
Wave 2 (Major fixes — parallel, no shared files between peers)
┌──────────────┴───────────────┐  ┌──────────────────────────────┐
│ 03-hot-operation-refactor    │  │ 04-source-ref-reverse-index  │
│ H3, L8 — handlers.rs        │  │ H4 — stack.rs                │
└──────────────────────────────┘  └──────────────────────────────┘
┌──────────────────────────────┐  ┌──────────────────────────────┐
│ 05-variables-correctness     │  │ 06-events-error-handling     │
│ M6, L4, L5 — variables.rs   │  │ M8, L1 — events.rs           │
└──────────────┬───────────────┘  └──────────────┬───────────────┘
               │                                 │
Wave 3 (Cleanup + performance — parallel)        │
┌──────────────┴───────────────┐  ┌──────────────┴───────────────┐
│ 08-tostring-getter-budgets   │  │ 09-varstore-cap-feedback     │
│ M2 — variables.rs            │  │ M4 — stack.rs, events.rs     │
└──────────────────────────────┘  └──────────────────────────────┘
┌──────────────────────────────┐
│ 07-adapter-module-cleanup    │
│ L2,L3,L7,L10,L11            │
│ mod.rs,types.rs,backend.rs,  │
│ dap_backend.rs               │
└──────────────────────────────┘

Wave 4 (Security hardening — parallel)
┌──────────────────────────────┐  ┌──────────────────────────────┐
│ 10-dap-server-auth           │  │ 11-callservice-security      │
│ M1, L9 — server/*.rs        │  │ M3 — handlers.rs, docs       │
└──────────────────────────────┘  └──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-handlers-critical-fixes](tasks/01-handlers-critical-fixes.md) | Done | - | 1–2h | `fdemon-dap/adapter/handlers.rs` |
| 2 | [02-evaluate-injection-fix](tasks/02-evaluate-injection-fix.md) | Done | - | 1–2h | `fdemon-dap/adapter/evaluate.rs` |
| 3 | [03-hot-operation-refactor](tasks/03-hot-operation-refactor.md) | Done (concern: line count -3 not -35, but duplication eliminated) | 1 | 1–2h | `fdemon-dap/adapter/handlers.rs` |
| 4 | [04-source-ref-reverse-index](tasks/04-source-ref-reverse-index.md) | Done | - | 1–2h | `fdemon-dap/adapter/stack.rs` |
| 5 | [05-variables-correctness](tasks/05-variables-correctness.md) | Done | - | 1–2h | `fdemon-dap/adapter/variables.rs` |
| 6 | [06-events-error-handling](tasks/06-events-error-handling.md) | Done | - | 0.5–1h | `fdemon-dap/adapter/events.rs` |
| 7 | [07-adapter-module-cleanup](tasks/07-adapter-module-cleanup.md) | Done | - | 1–2h | `fdemon-dap/adapter/mod.rs`, `types.rs`, `backend.rs`, `fdemon-app/handler/dap_backend.rs` |
| 8 | [08-tostring-getter-budgets](tasks/08-tostring-getter-budgets.md) | Done | 5 | 1–2h | `fdemon-dap/adapter/variables.rs` |
| 9 | [09-varstore-cap-feedback](tasks/09-varstore-cap-feedback.md) | Done | 4, 6 | 1–2h | `fdemon-dap/adapter/stack.rs`, `fdemon-dap/adapter/events.rs` |
| 10 | [10-dap-server-auth](tasks/10-dap-server-auth.md) | Done (concern: missing behavioral test for attached idle timeout) | - | 2–3h | `fdemon-dap/server/mod.rs`, `fdemon-dap/server/session.rs` |
| 11 | [11-callservice-security](tasks/11-callservice-security.md) | Done | 3 | 0.5–1h | `fdemon-dap/adapter/handlers.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01-handlers-critical-fixes | `fdemon-dap/adapter/handlers.rs` | `fdemon-dap/adapter/breakpoints.rs` |
| 02-evaluate-injection-fix | `fdemon-dap/adapter/evaluate.rs` | `fdemon-dap/adapter/variables.rs` (enrich_with_to_string pattern) |
| 03-hot-operation-refactor | `fdemon-dap/adapter/handlers.rs` | `fdemon-dap/adapter/events.rs` |
| 04-source-ref-reverse-index | `fdemon-dap/adapter/stack.rs` | - |
| 05-variables-correctness | `fdemon-dap/adapter/variables.rs` | - |
| 06-events-error-handling | `fdemon-dap/adapter/events.rs` | - |
| 07-adapter-module-cleanup | `fdemon-dap/adapter/mod.rs`, `fdemon-dap/adapter/types.rs`, `fdemon-dap/adapter/backend.rs`, `fdemon-app/handler/dap_backend.rs` | - |
| 08-tostring-getter-budgets | `fdemon-dap/adapter/variables.rs` | - |
| 09-varstore-cap-feedback | `fdemon-dap/adapter/stack.rs`, `fdemon-dap/adapter/events.rs` | - |
| 10-dap-server-auth | `fdemon-dap/server/mod.rs`, `fdemon-dap/server/session.rs`, `fdemon-dap/protocol/types.rs` | - |
| 11-callservice-security | `fdemon-dap/adapter/handlers.rs` | - |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | None | Parallel (worktree) |
| 03 + 04 | None | Parallel (worktree) |
| 03 + 05 | None | Parallel (worktree) |
| 03 + 06 | None | Parallel (worktree) |
| 04 + 05 | None | Parallel (worktree) |
| 04 + 06 | None | Parallel (worktree) |
| 05 + 06 | None | Parallel (worktree) |
| 01 + 03 | `handlers.rs` | Sequential (same branch) |
| 05 + 08 | `variables.rs` | Sequential (same branch) |
| 04 + 09 | `stack.rs` | Sequential (same branch) |
| 06 + 09 | `events.rs` | Sequential (same branch) |
| 03 + 11 | `handlers.rs` | Sequential (same branch) |
| 10 + 11 | None | Parallel (worktree) |
| 07 + 08 | None | Parallel (worktree) |
| 07 + 09 | None | Parallel (worktree) |

## Success Criteria

Phase 6 fixes are complete when:

- [ ] No `.expect()` or `.unwrap()` in non-test adapter code without `// SAFETY:` justification
- [ ] Hover evaluate does not embed raw user expressions in Dart code strings
- [ ] `restart` and `hotRestart` requests produce identical progress events and state invalidation
- [ ] `column=0` in completions returns an error, not wrong results
- [ ] `SourceReferenceStore::get_or_create` is O(1) lookup
- [ ] Map key evaluateNames produce valid Dart for keys with `"`, `\`, `$`, `\n`
- [ ] Failed auto-resume is logged at warn level
- [ ] toString/getter evaluation loops have a global time budget
- [ ] Variable store cap emits IDE-visible feedback
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` clean

## Notes

- **All Wave 1–2 tasks touch different files** within each wave and can safely run in parallel worktrees.
- **File-based dependency chains:** `handlers.rs`: 01 → 03 → 11; `variables.rs`: 05 → 08; `stack.rs`: 04 → 09; `events.rs`: 06 → 09.
- **Wave 4 (security) tasks are lower priority** and can be deferred if needed — they address defense-in-depth for a localhost-only debug server.
- **Existing tests are extensive** (~9,300 lines). New tests for each fix should follow the same patterns in `crates/fdemon-dap/src/adapter/tests/`.
