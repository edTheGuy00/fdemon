# Phase 2: Update Runner to Use Message-Based Auto-Start - Task Index

## Overview

Remove synchronous startup logic from the runner and use the message-based auto-start infrastructure created in Phase 1. After this phase, the app will always enter Normal mode first, then trigger auto-start via message if configured.

**Total Tasks:** 3
**Estimated Hours:** 2-3 hours

## Task Dependency Graph

```
┌─────────────────────────────────┐
│  01-simplify-startup-flutter    │
└───────────────┬─────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│  02-update-runner               │
└───────────────┬─────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│  03-verify-animation            │
└─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-simplify-startup-flutter](tasks/01-simplify-startup-flutter.md) | Not Started | Phase 1 | 1h | `tui/startup.rs` |
| 2 | [02-update-runner](tasks/02-update-runner.md) | Not Started | 1 | 1h | `tui/runner.rs` |
| 3 | [03-verify-animation](tasks/03-verify-animation.md) | Not Started | 2 | 0.5h | (verification only) |

## Success Criteria

Phase 2 is complete when:

- [ ] `startup_flutter()` always enters Normal mode (no branching on `auto_start`)
- [ ] `startup_flutter()` returns `StartupAction` enum indicating next step
- [ ] `runner.rs` no longer sets loading state before the loop
- [ ] `runner.rs` sends `StartAutoLaunch` message after first render (when `auto_start=true`)
- [ ] Loading animation works correctly via `Message::Tick`
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes
- [ ] Manual verification: auto-start flow shows Normal (brief) → Loading → Running

## Notes

- After this phase, the sync `auto_start_session()` logic in `startup.rs` becomes dead code
- Phase 3 will complete the async task implementation
- Phase 4 will clean up dead code
- This is the "breaking change" phase - auto-start behavior shifts from sync to async
