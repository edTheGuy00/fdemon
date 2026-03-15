# Pre-App Custom Sources — Phase 2: Shared Custom Sources

## Overview

Add `shared = true` config option so custom sources can be spawned once and shared across all Flutter sessions. Prevents port conflicts and redundant processes when running multi-device sessions against a single backend.

**Total Tasks:** 10
**Source:** [PLAN.md](../PLAN.md) — Phase 2

## Task Dependency Graph

```
Wave 1 (parallel — no deps, foundation)
┌───────────────────────────┐  ┌──────────────────────────────┐  ┌───────────────────────────────┐
│ 01-config-shared-field    │  │ 02-shared-source-handle      │  │ 03-message-variants           │
│ Add shared bool to config │  │ SharedSourceHandle + AppState │  │ SharedSourceLog/Started/Stop  │
└───────────────────────────┘  └──────────────────────────────┘  └───────────────────────────────┘

Wave 2 (parallel — depends on Wave 1)
┌───────────────────────────┐  ┌──────────────────────────────┐
│ 04-tea-handlers           │  │ 05-spawn-shared-pre-app      │
│ Handle new Message types  │  │ Modify spawn_pre_app_sources │
└───────────────────────────┘  └──────────────────────────────┘

Wave 3 (depends on Wave 2)
┌───────────────────────────────┐
│ 06-spawn-shared-post-app      │
│ Modify spawn_custom_sources   │
└───────────────────────────────┘

Wave 4 (parallel — depends on Wave 2-3)
┌───────────────────────────┐  ┌──────────────────────────────┐
│ 07-pre-app-gate-skip      │  │ 08-engine-shutdown           │
│ Skip ready check if alive │  │ Shared source cleanup        │
└───────────────────────────┘  └──────────────────────────────┘

Wave 5 (parallel — depends on all)
┌───────────────────────────┐  ┌──────────────────────────────┐
│ 09-integration-tests      │  │ 10-documentation             │
│ Multi-session test suite  │  │ CONFIGURATION.md + ARCH.md   │
└───────────────────────────┘  └──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-config-shared-field](tasks/01-config-shared-field.md) | Not Started | - | `config/types.rs` |
| 2 | [02-shared-source-handle](tasks/02-shared-source-handle.md) | Not Started | - | `session/handle.rs`, `state.rs` |
| 3 | [03-message-variants](tasks/03-message-variants.md) | Not Started | - | `message.rs` |
| 4 | [04-tea-handlers](tasks/04-tea-handlers.md) | Not Started | 1, 2, 3 | `handler/update.rs` |
| 5 | [05-spawn-shared-pre-app](tasks/05-spawn-shared-pre-app.md) | Not Started | 1, 2, 3 | `actions/native_logs.rs`, `actions/mod.rs` |
| 6 | [06-spawn-shared-post-app](tasks/06-spawn-shared-post-app.md) | Not Started | 5 | `actions/native_logs.rs` |
| 7 | [07-pre-app-gate-skip](tasks/07-pre-app-gate-skip.md) | Not Started | 4, 5 | `handler/new_session/launch_context.rs`, `handler/update.rs` |
| 8 | [08-engine-shutdown](tasks/08-engine-shutdown.md) | Not Started | 2, 4 | `engine.rs` |
| 9 | [09-integration-tests](tasks/09-integration-tests.md) | Not Started | 4, 5, 6, 7, 8 | `handler/tests.rs` |
| 10 | [10-documentation](tasks/10-documentation.md) | Not Started | 1 | `docs/CONFIGURATION.md`, `docs/ARCHITECTURE.md` |

## Success Criteria

Phase 2 is complete when:

- [ ] `shared = true` custom source spawns once across all sessions
- [ ] Shared source logs are broadcast to all active sessions
- [ ] Shared source survives individual session close
- [ ] Shared source cleaned up only on engine shutdown / fdemon quit
- [ ] Second session skips ready check for already-running shared source
- [ ] Non-shared sources remain per-session (no regression)
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings`

## Notes

- All module paths are relative to `crates/fdemon-app/src/`
- The TEA loop is single-threaded, so no race conditions on `shared_source_handles` access
- `SharedSourceLog` broadcasts require iterating all sessions — acceptable overhead since log events are already O(1) per session
- The `file_watcher` pattern (Engine-level, spawned once, bridged to `msg_tx`) is the closest existing analogue
