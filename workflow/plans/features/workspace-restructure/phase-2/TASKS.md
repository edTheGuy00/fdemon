# Phase 2: Extract Engine and Wire Services - Task Index

## Overview

Create a shared `Engine` abstraction that both TUI and headless runners use, eliminating duplicated orchestration code. Wire the dormant services layer (`FlutterController`, `LogService`, `StateService`) into the actual runtime via the Engine, and add an event broadcasting system for future pro feature extensibility.

**Total Tasks:** 7
**Estimated Hours:** 24-34 hours

## Task Dependency Graph

```
┌─────────────────────────────┐  ┌─────────────────────────────┐
│ 01-define-engine-struct     │  │ 02-define-engine-event      │
│ (Engine struct + new())     │  │ (EngineEvent enum)          │
└────────────┬────────────────┘  └────────────┬────────────────┘
             │                                │
             └──────────┬─────────────────────┘
                        ▼
          ┌─────────────────────────────┐
          │ 03-refactor-tui-runner      │
          │ (TUI uses Engine)           │
          └────────────┬────────────────┘
                       │
                       ▼
          ┌─────────────────────────────┐
          │ 04-refactor-headless-runner │
          │ (Headless uses Engine)      │
          └────────────┬────────────────┘
                       │
                       ▼
          ┌─────────────────────────────┐
          │ 05-wire-services-layer      │
          │ (SharedState sync + traits) │
          └────────────┬────────────────┘
                       │
                       ▼
          ┌─────────────────────────────┐
          │ 06-add-event-broadcasting   │
          │ (Engine.subscribe())        │
          └────────────┬────────────────┘
                       │
                       ▼
          ┌─────────────────────────────┐
          │ 07-verify-and-document      │
          │ (tests, docs, cleanup)      │
          └─────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Key Modules |
|---|------|--------|------------|------------|-------------|
| 1 | [01-define-engine-struct](tasks/01-define-engine-struct.md) | Done | - | 4-5h | `app/engine.rs` **NEW** |
| 2 | [02-define-engine-event](tasks/02-define-engine-event.md) | Done | - | 2-3h | `app/engine_event.rs` **NEW** |
| 3 | [03-refactor-tui-runner](tasks/03-refactor-tui-runner.md) | Done | 1, 2 | 5-7h | `tui/runner.rs`, `tui/startup.rs` |
| 4 | [04-refactor-headless-runner](tasks/04-refactor-headless-runner.md) | Done | 3 | 4-6h | `headless/runner.rs`, `headless/mod.rs` |
| 5 | [05-wire-services-layer](tasks/05-wire-services-layer.md) | Done | 4 | 4-5h | `services/`, `app/engine.rs` |
| 6 | [06-add-event-broadcasting](tasks/06-add-event-broadcasting.md) | Done | 5 | 3-4h | `app/engine.rs`, `app/engine_event.rs` |
| 7 | [07-verify-and-document](tasks/07-verify-and-document.md) | Done | 6 | 2-4h | `docs/ARCHITECTURE.md` |

## Success Criteria

Phase 2 is complete when:

- [ ] `Engine` struct exists in `app/engine.rs` and encapsulates shared orchestration
- [ ] TUI runner creates an `Engine` and delegates all channel/state management to it
- [ ] Headless runner creates an `Engine` and delegates all channel/state management to it
- [ ] No duplicated orchestration code between TUI and headless runners
- [ ] Headless `spawn_headless_session()` is eliminated (uses `Engine` -> `app/actions::spawn_session`)
- [ ] Services layer (`FlutterController`, `LogService`, `StateService`) is wired into `Engine`
- [ ] `SharedState` is synchronized from `AppState` after each message batch
- [ ] `Engine.subscribe()` returns a `broadcast::Receiver<EngineEvent>` for external consumers
- [ ] `cargo test` passes with no regressions
- [ ] `cargo clippy` is clean
- [ ] `docs/ARCHITECTURE.md` accurately reflects the Engine-based architecture

## Notes

- **Tasks 1 and 2 are independent** and can be done in parallel (Wave 1)
- **Task 3** depends on both because the TUI runner needs the Engine struct and EngineEvent types
- **Task 4** depends on Task 3 because the headless refactor should follow the same pattern established by TUI
- **Tasks 5, 6** are sequential -- services must be wired before broadcasting can emit service events
- **Task 7** is the final verification pass
- All tasks operate within the single-crate structure (Phase 3 does the workspace split)
- The Engine must NOT depend on ratatui or any TUI-specific types
- The headless runner's `spawn_headless_session()` (~160 lines) duplicates `app/actions::spawn_session()` and must be eliminated
