# Phase 4 Followup: Review Action Items - Task Index

## Overview

Address all issues identified in the phase-4 code review (`workflow/reviews/features/workspace-restructure-phase-4/ACTION_ITEMS.md`). Two critical blockers (runtime panics, silent API failures), four major issues (dead code masking, unnecessary clones, unused statics, clippy gate), and four minor items (debug logging, doc ordering, test-only code, re-exports).

**Total Tasks:** 7
**Source:** Phase-4 review verdict: APPROVED WITH CONCERNS

## Task Dependency Graph

```
Wave 1 (parallel - independent fixes):
┌──────────────────────────────┐  ┌──────────────────────────────┐  ┌──────────────────────────────┐
│  01-remove-startup-dead-code │  │  02-restrict-dispatch-action │  │  03-clean-handler-dead-code  │
│  CRITICAL: unimplemented!()  │  │  CRITICAL: silent failures   │  │  MAJOR: blanket allows       │
└──────────────┬───────────────┘  └──────────────┬───────────────┘  └──────────────┬───────────────┘
               │                                 │                                 │
               └─────────────────┬───────────────┴─────────────────────────────────┘
                                 ▼
Wave 2 (parallel - minor fixes):
┌──────────────────────────────┐  ┌──────────────────────────────┐  ┌──────────────────────────────┐
│  04-remove-dead-statics      │  │  05-guard-plugin-clone       │  │  06-minor-cleanups           │
│  MAJOR: PACKAGE_PATH_REGEX   │  │  MAJOR: unconditional clone  │  │  MINOR: 4 small fixes        │
│  + has_flutter_dependency    │  │                              │  │                              │
└──────────────┬───────────────┘  └──────────────┬───────────────┘  └──────────────┬───────────────┘
               │                                 │                                 │
               └─────────────────┬───────────────┴─────────────────────────────────┘
                                 ▼
Wave 3 (final verification):
┌──────────────────────────────┐
│  07-verify-clippy-clean      │
│  Full quality gate check     │
└──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Severity | Key Modules |
|---|------|--------|------------|----------|-------------|
| 1 | [01-remove-startup-dead-code](tasks/01-remove-startup-dead-code.md) | Not Started | - | CRITICAL | `crates/fdemon-tui/src/startup.rs` |
| 2 | [02-restrict-dispatch-action](tasks/02-restrict-dispatch-action.md) | Not Started | - | CRITICAL | `crates/fdemon-app/src/engine.rs` |
| 3 | [03-clean-handler-dead-code](tasks/03-clean-handler-dead-code.md) | Not Started | - | MAJOR | `crates/fdemon-app/src/handler/` |
| 4 | [04-remove-dead-statics](tasks/04-remove-dead-statics.md) | Not Started | - | MAJOR | `crates/fdemon-core/src/`, `crates/fdemon-app/src/` |
| 5 | [05-guard-plugin-clone](tasks/05-guard-plugin-clone.md) | Not Started | - | MAJOR | `crates/fdemon-app/src/engine.rs` |
| 6 | [06-minor-cleanups](tasks/06-minor-cleanups.md) | Not Started | - | MINOR | Multiple |
| 7 | [07-verify-clippy-clean](tasks/07-verify-clippy-clean.md) | Not Started | 1, 2, 3, 4, 5, 6 | GATE | All crates |

## Success Criteria

Phase 4 followup is complete when:

- [ ] No `unimplemented!()` calls in production code
- [ ] No blanket `#[allow(dead_code)]` on modules
- [ ] `dispatch_action()` has documented limitations or restricted signature
- [ ] `cargo fmt --all` -- formatted
- [ ] `cargo check --workspace` -- compiles
- [ ] `cargo test --workspace --lib` -- all tests pass
- [ ] `cargo clippy --workspace -- -D warnings` -- clean (no warnings)

## Notes

- Tasks 1-6 can all run in parallel (independent modules)
- Task 7 is the final gate check and depends on all others
- Wave 1 contains the two critical blockers from the review
- Wave 2 items are important but non-blocking quality improvements
