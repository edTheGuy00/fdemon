# DAP Server Phase 3 Fixes — Task Index

## Overview

Address the 3 critical, 4 major, and 8 minor issues identified in the [Phase 3 review](../../../../reviews/features/dap-server-phase-3/REVIEW.md). The primary blocker is that the entire DAP feature is non-functional for real debugging: both TCP and stdio transport hardcode `NoopBackend`, so every `attach` request fails.

**Total Tasks:** 9
**Estimated Hours:** 18–26 hours

## Task Dependency Graph

```
Wave 1 (Critical — parallel)
┌────────────────────────────┐   ┌────────────────────────────┐
│  01-wire-tcp-backend       │   │  02-fix-stdio-busy-poll    │
└─────────────┬──────────────┘   └─────────────┬──────────────┘
              │                                │
Wave 2        │                                │
              ▼                                │
┌────────────────────────────┐                 │
│  03-fix-stdio-mode         │                 │
└─────────────┬──────────────┘                 │
              │                                │
Wave 3        └──────────┬─────────────────────┘
                         ▼
              ┌────────────────────────────────┐
              │  04-consolidate-session-loops   │
              └────────────────────────────────┘

Wave 4 (Major — parallel, no dependencies)
┌─────────────────────────────────┐  ┌─────────────────────────────────┐
│  05-fix-disconnect-terminated   │  │  06-replace-eprintln            │
└─────────────────────────────────┘  └─────────────────────────────────┘
┌─────────────────────────────────┐
│  07-fix-dart-uri-to-path        │
└─────────────────────────────────┘

Wave 5 (Minor — parallel, no dependencies)
┌─────────────────────────────────┐  ┌─────────────────────────────────┐
│  08-code-quality-cleanup        │  │  09-type-safety-improvements    │
└─────────────────────────────────┘  └─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-wire-tcp-backend](tasks/01-wire-tcp-backend.md) | Not Started | - | 4–6h | `fdemon-dap/src/server/mod.rs`, `service.rs`, `fdemon-app/src/actions/mod.rs` |
| 2 | [02-fix-stdio-busy-poll](tasks/02-fix-stdio-busy-poll.md) | Not Started | - | 1–2h | `fdemon-dap/src/server/session.rs`, `transport/stdio.rs` |
| 3 | [03-fix-stdio-mode](tasks/03-fix-stdio-mode.md) | Not Started | 1 | 2–3h | `src/dap_stdio/runner.rs`, `docs/IDE_SETUP.md` |
| 4 | [04-consolidate-session-loops](tasks/04-consolidate-session-loops.md) | Not Started | 1, 2 | 3–4h | `fdemon-dap/src/server/session.rs` |
| 5 | [05-fix-disconnect-terminated](tasks/05-fix-disconnect-terminated.md) | Not Started | - | 1–2h | `fdemon-dap/src/server/session.rs`, `adapter/mod.rs` |
| 6 | [06-replace-eprintln](tasks/06-replace-eprintln.md) | Not Started | - | 1–2h | `fdemon-app/src/actions/mod.rs` |
| 7 | [07-fix-dart-uri-to-path](tasks/07-fix-dart-uri-to-path.md) | Not Started | - | 1–2h | `fdemon-dap/src/adapter/stack.rs` |
| 8 | [08-code-quality-cleanup](tasks/08-code-quality-cleanup.md) | Not Started | - | 2–3h | Multiple files (6 minor fixes) |
| 9 | [09-type-safety-improvements](tasks/09-type-safety-improvements.md) | Not Started | - | 2–3h | `fdemon-dap/src/adapter/mod.rs`, `fdemon-app/src/handler/dap_backend.rs` |

## Success Criteria

Phase 3 Fixes are complete when:

- [ ] IDE connects via TCP → `attach` succeeds against a running Flutter session → `threads` returns isolates
- [ ] No CPU spin in stdio sessions (broadcast `Closed` handled correctly)
- [ ] Stdio mode either works end-to-end OR docs are updated with honest limitations
- [ ] Single implementation of the DAP session event loop (no duplicated select logic)
- [ ] `terminated` event emitted on client-initiated disconnect
- [ ] No `eprintln!` calls in library code
- [ ] `dart_uri_to_path` handles Windows paths or has explicit platform guard
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` passes
- [ ] `cargo fmt --all` produces no changes

## Notes

- **Layer boundary preserved**: `fdemon-dap` must NOT depend on `fdemon-app`. The backend factory crosses this boundary via a trait object or closure, constructed in `fdemon-app`.
- **Task 01 is the linchpin**: Most other tasks can proceed independently, but task 04 (consolidate loops) should wait until after tasks 01 and 02 since both modify the session event loop.
- **Minor fixes are batched**: Tasks 08 and 09 each group several small, independent fixes to reduce overhead.
