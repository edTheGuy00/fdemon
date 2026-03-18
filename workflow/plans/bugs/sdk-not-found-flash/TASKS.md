# SDK Not Found Flash — Task Index

## Overview

Fix the regression where "Flutter SDK not found" flashes momentarily in the new session dialog on startup. Two-part fix: restore PATH-based fallback with distinct SdkSource variant, and prevent bootable device discovery from clearing SDK errors.

**Total Tasks:** 2
**Bug Report:** `workflow/plans/bugs/sdk-not-found-flash/BUG.md`

## Task Dependency Graph

```
┌────────────────────────────────────┐
│  01-restore-path-fallback          │
│  (locator.rs + types.rs)           │
└────────────────┬───────────────────┘
                 │
                 ▼
┌────────────────────────────────────┐
│  02-fix-error-clearing             │
│  (target_selector_state.rs)        │
└────────────────────────────────────┘
```

### Parallelism

| Wave | Tasks | Can Run In Parallel |
|------|-------|-------------------|
| 1 | 01 | No |
| 2 | 02 | No (depends on 01 for end-to-end verification) |

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-restore-path-fallback](tasks/01-restore-path-fallback.md) | Done | - | `fdemon-daemon: flutter_sdk/locator.rs, flutter_sdk/types.rs` |
| 2 | [02-fix-error-clearing](tasks/02-fix-error-clearing.md) | Done | 01 | `fdemon-app: new_session_dialog/target_selector_state.rs` `fdemon-tui: widgets/new_session_dialog/target_selector.rs` |

## Success Criteria

Bug fix is complete when:

- [ ] `find_flutter_sdk` returns `Ok(sdk)` with `SdkSource::PathInferred` when binary is on PATH but SDK root can't be fully resolved
- [ ] `set_bootable_devices()` does NOT clear `target_selector.error`
- [ ] No "Flutter SDK not found" flash on startup when Flutter is available on PATH
- [ ] When Flutter is genuinely absent, the error persists correctly
- [ ] All existing tests pass
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

## Notes

- Task 01 addresses the primary regression (bare PATH fallback removal from Phase 1 fixes Task 02).
- Task 02 is a defense-in-depth fix — even if the SDK is genuinely missing, the error should not be silently cleared by bootable device discovery.
- Both tasks touch different crates but are sequenced for end-to-end verification.
