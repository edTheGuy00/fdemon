# Phase 4: Cleanup and Final Verification - Task Index

## Overview

Remove dead code from `startup.rs`, update documentation, and perform final verification. This phase completes the startup flow consistency feature.

**Total Tasks:** 4
**Estimated Hours:** 2-3 hours

## Task Dependency Graph

```
┌─────────────────────────────────┐
│  01-remove-dead-code            │
└───────────────┬─────────────────┘
                │
                ▼
┌─────────────────────────────────┐     ┌─────────────────────────────────┐
│  02-update-snapshot-tests       │     │  03-update-documentation        │
└───────────────┬─────────────────┘     └───────────────┬─────────────────┘
                │                                       │
                └──────────────┬────────────────────────┘
                               ▼
                ┌─────────────────────────────────┐
                │  04-final-verification          │
                └─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-remove-dead-code](tasks/01-remove-dead-code.md) | Not Started | Phase 3 | 1h | `tui/startup.rs` |
| 2 | [02-update-snapshot-tests](tasks/02-update-snapshot-tests.md) | Not Started | 1 | 0.5h | `tui/render/tests.rs` |
| 3 | [03-update-documentation](tasks/03-update-documentation.md) | Not Started | 1 | 0.5h | `docs/`, `CLAUDE.md` |
| 4 | [04-final-verification](tasks/04-final-verification.md) | Not Started | 2, 3 | 0.5h | (verification only) |

## Success Criteria

Phase 4 (and entire feature) is complete when:

- [ ] All dead code removed from `startup.rs`
- [ ] No dead code warnings from `cargo clippy`
- [ ] Snapshot tests updated/passing
- [ ] Documentation updated (ARCHITECTURE.md startup sequence)
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes
- [ ] Final manual E2E verification passes

## Notes

- This is the final phase - feature is complete after this
- Tasks 2 and 3 can be done in parallel
- Keep the cleanup focused - don't refactor beyond what's necessary
- Update PLAN.md with completion status when done

## Phase 3 Impact Summary

Phase 3 made the following changes that affect Phase 4:

| Change | Impact on Phase 4 |
|--------|-------------------|
| Loading screen is now a modal overlay | Documentation (Task 03) should describe overlay behavior |
| Loading snapshot updated | Task 02 partially complete - verify other snapshots |
| `DevicesDiscovered` no longer transitions UI | No impact on cleanup tasks |
| Message cycling enabled | Documentation should mention message cycling |

**Task 02 note:** The loading snapshot was already updated during Phase 3 manual verification. Only need to verify other snapshots still pass.
