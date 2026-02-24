# Phase 2 Fixes: Review Remediation - Task Index

## Overview

Address 2 critical, 4 major, and 6 minor issues identified during the phase-2 code review. The dart defines modal saves on Esc instead of discarding, both modal open handlers lack mutual exclusion guards, magic string literals are scattered across 5+ files, and several minor state-management edge cases need cleanup.

**Review:** `workflow/reviews/features/v1-refinements-phase-2/REVIEW.md`
**Total Tasks:** 6
**Estimated Hours:** 7-10 hours

## Task Dependency Graph

```
┌──────────────────────┐  ┌──────────────────────┐  ┌──────────────────────┐
│ 01-dart-defines      │  │ 02-modal-open        │  │ 03-magic-string      │
│ -cancel              │  │ -guard               │  │ -constants           │
│ (Critical #1)        │  │ (Critical #2)        │  │ (Major #3)           │
└──────────┬───────────┘  └──────────┬───────────┘  └──────────┬───────────┘
           │                         │                         │
┌──────────┼─────────────────────────┼─────────────────────────┘
│          │              ┌──────────────────────┐  ┌──────────────────────┐
│          │              │ 04-extra-args        │  │ 05-modal-state       │
│          │              │ -empty-confirm       │  │ -cleanup             │
│          │              │ (Major #6)           │  │ (Minor #7, #8)       │
│          │              └──────────┬───────────┘  └──────────┬───────────┘
│          │                         │                         │
└──────────┴─────────────────────────┴─────────────────────────┘
                                     │
                          ┌──────────────────────┐
                          │ 06-review-fix        │
                          │ -tests               │
                          │ (depends on: all)    │
                          └──────────────────────┘
```

**Execution waves:**
- **Wave 1** (parallel): 01-dart-defines-cancel, 02-modal-open-guard, 03-magic-string-constants, 04-extra-args-empty-confirm, 05-modal-state-cleanup
- **Wave 2** (after all): 06-review-fix-tests

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-dart-defines-cancel](tasks/01-dart-defines-cancel.md) | Done | - | 2-3h | `settings_dart_defines.rs`, `keys.rs`, `message.rs` |
| 2 | [02-modal-open-guard](tasks/02-modal-open-guard.md) | Done | - | 0.5-1h | `settings_dart_defines.rs`, `settings_extra_args.rs` |
| 3 | [03-magic-string-constants](tasks/03-magic-string-constants.md) | Done | - | 1-2h | `settings_items.rs`, `settings_handlers.rs`, `settings.rs` |
| 4 | [04-extra-args-empty-confirm](tasks/04-extra-args-empty-confirm.md) | Done | - | 0.5-1h | `settings_extra_args.rs` |
| 5 | [05-modal-state-cleanup](tasks/05-modal-state-cleanup.md) | Done | - | 1-2h | `state.rs` |
| 6 | [06-review-fix-tests](tasks/06-review-fix-tests.md) | Done | 1, 2, 3, 4, 5 | 1.5-2h | Cross-module tests |

## Review Issue Mapping

| Review Issue | Severity | Task |
|---|---|---|
| #1 Esc persists dart defines | Critical | 01 |
| #2 No modal open guard | Critical | 02 |
| #3 Magic string literals | Major | 03 |
| #4 Silent data-loss path | Major | 01 |
| #5 Inaccurate doc comment | Major | 01 |
| #6 Extra args empty confirm | Major | 04 |
| #7 Shared editing_config_idx | Minor | 05 |
| #8 hide_settings() modal leak | Minor | 05 |
| #9 HashMap ordering | Minor | 01 |
| #10 Magic +1 constant | Minor | 03 |
| #11 PRESET_EXTRA_ARGS doc | Minor | 03 |

## Success Criteria

Phase 2 fixes are complete when:

- [ ] Esc in dart defines modal discards changes (does not persist)
- [ ] Both modal open handlers guard against simultaneous modals
- [ ] Magic strings replaced with named constants
- [ ] Extra args confirm with no selection keeps modal open
- [ ] `hide_settings()` clears modal state
- [ ] All new tests pass
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

## Notes

- Tasks 01 and 02 both touch `settings_dart_defines.rs` but different functions (cancel handler vs. open guard). Minimal merge conflict risk.
- Task 03 touches the same files as 01/02 but only string literal sites, not handler logic. Can be parallelized safely.
- The TEA purity violation (Review #7 — blocking I/O in update) is tracked as pre-existing tech debt and intentionally deferred.
- File size warning (Review #12 — files approaching 500 lines) is deferred to the next feature increment.
- Repeated disk reads on keypress (Review #11 in REVIEW.md) is deferred as a performance optimization.
