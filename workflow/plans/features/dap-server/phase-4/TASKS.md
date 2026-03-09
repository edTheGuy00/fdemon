# DAP Server Phase 4: Flutter Integration & Polish — Task Index

## Overview

Complete the DAP debugging experience with debug event routing (fixing the broken pause/resume flow), hot reload/restart integration, conditional breakpoints, logpoints, expression evaluation, source references, multi-session support, and production hardening. The critical blocker is that VM Service debug events (stopped, continued, thread lifecycle) never reach the DAP adapter because the per-session event channel sender is dropped immediately after creation.

**Total Tasks:** 12
**Estimated Hours:** 38–54 hours

## Task Dependency Graph

```
Wave 1 (Critical — the linchpin fix)
┌──────────────────────────────────────┐
│  01-wire-debug-event-channel         │
│  Fix pause/resume/exception stuck    │
└─────────────────┬────────────────────┘
                  │
Wave 2 (Core features — parallel, depend on 01)
┌─────────────────┴────────────────────┐  ┌──────────────────────────────┐
│  02-hot-reload-restart-dap           │  │  03-coordinated-pause        │
│  Custom DAP requests for reload      │  │  Suppress auto-reload        │
└─────────────────┬────────────────────┘  └──────────────┬───────────────┘
                  │                                      │
Wave 3 (Features — parallel, depend on 01)
┌─────────────────┴────────────────────┐  ┌──────────────┴───────────────┐
│  04-conditional-breakpoints          │  │  05-logpoints                │
└──────────────────────────────────────┘  └──────────────────────────────┘
┌──────────────────────────────────────┐  ┌──────────────────────────────┐
│  06-expression-eval-enhancements     │  │  07-source-references        │
└──────────────────────────────────────┘  └──────────────────────────────┘

Wave 4 (Integration — parallel, depend on 01)
┌──────────────────────────────────────┐  ┌──────────────────────────────┐
│  08-custom-dap-events                │  │  09-multi-session-threads    │
└──────────────────────────────────────┘  └──────────────────────────────┘

Wave 5 (Hardening — parallel, depend on 02)
┌──────────────────────────────────────┐  ┌──────────────────────────────┐
│  10-breakpoint-persistence           │  │  11-production-hardening     │
└──────────────────────────────────────┘  └──────────────────────────────┘
┌──────────────────────────────────────┐
│  12-documentation-update             │
└──────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-wire-debug-event-channel](tasks/01-wire-debug-event-channel.md) | Done | - | 5–7h | `fdemon-app/handler/dap_backend.rs`, `fdemon-app/handler/devtools/debug.rs`, `fdemon-app/engine.rs`, `fdemon-app/session.rs`, `fdemon-dap/server/session.rs` |
| 2 | [02-hot-reload-restart-dap](tasks/02-hot-reload-restart-dap.md) | Done | 1 | 3–4h | `fdemon-dap/adapter/mod.rs`, `fdemon-app/handler/dap_backend.rs` |
| 3 | [03-coordinated-pause](tasks/03-coordinated-pause.md) | Done | 1 | 3–4h | `fdemon-app/handler/devtools/debug.rs`, `fdemon-app/engine.rs`, `fdemon-app/watcher.rs` |
| 4 | [04-conditional-breakpoints](tasks/04-conditional-breakpoints.md) | Done | 1 | 3–5h | `fdemon-dap/adapter/breakpoints.rs`, `fdemon-dap/adapter/mod.rs` |
| 5 | [05-logpoints](tasks/05-logpoints.md) | Done | 1 | 3–4h | `fdemon-dap/adapter/breakpoints.rs`, `fdemon-dap/adapter/mod.rs` |
| 6 | [06-expression-eval-enhancements](tasks/06-expression-eval-enhancements.md) | Done | 1 | 3–4h | `fdemon-dap/adapter/evaluate.rs` |
| 7 | [07-source-references](tasks/07-source-references.md) | Done | 1 | 3–5h | `fdemon-dap/adapter/stack.rs`, `fdemon-dap/adapter/mod.rs` |
| 8 | [08-custom-dap-events](tasks/08-custom-dap-events.md) | Done | 1 | 2–3h | `fdemon-dap/adapter/mod.rs`, `fdemon-dap/server/session.rs` |
| 9 | [09-multi-session-threads](tasks/09-multi-session-threads.md) | Done | 1 | 4–6h | `fdemon-dap/adapter/threads.rs`, `fdemon-dap/adapter/mod.rs`, `fdemon-app/handler/dap_backend.rs` |
| 10 | [10-breakpoint-persistence](tasks/10-breakpoint-persistence.md) | Done | 2 | 3–4h | `fdemon-dap/adapter/breakpoints.rs`, `fdemon-dap/adapter/mod.rs` |
| 11 | [11-production-hardening](tasks/11-production-hardening.md) | Done | 2 | 3–5h | `fdemon-dap/server/session.rs`, `fdemon-dap/adapter/mod.rs`, `fdemon-dap/server/mod.rs` |
| 12 | [12-documentation-update](tasks/12-documentation-update.md) | Done | 2 | 2–3h | `docs/IDE_SETUP.md`, `docs/ARCHITECTURE.md` |

## Success Criteria

Phase 4 is complete when:

- [ ] Pause button in Zed transitions back to Play after pause/continue cycle
- [ ] Uncaught exceptions show in IDE with exception details, can be resumed
- [ ] `stopped` events sent for all pause reasons (breakpoint, exception, step, pause, entry)
- [ ] `continued` events sent after every resume (continue, step over/in/out)
- [ ] `thread` events sent for isolate start/exit
- [ ] Hot reload/restart work from custom DAP requests
- [ ] Breakpoints persist across hot restart
- [ ] Auto-reload suppressed while debugger is paused at breakpoint
- [ ] Conditional breakpoints evaluate conditions before stopping
- [ ] Logpoints log messages without stopping execution
- [ ] Expression evaluation works in hover, watch, repl, and clipboard contexts
- [ ] SDK/package sources viewable via `sourceReference` in IDE
- [ ] Custom DAP events sent (dart.debuggerUris, flutter.appStarted)
- [ ] Multi-session debugging: threads namespaced per session
- [ ] Connection timeout handling prevents zombie sessions
- [ ] Rate limiting on variable expansion prevents performance issues
- [ ] Graceful degradation when VM Service disconnects mid-debug
- [ ] Tested with Zed DAP client end-to-end
- [ ] 200+ new unit tests across all tasks
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` clean

## Notes

- **Task 01 is the critical blocker**: All debugging is non-functional without debug event routing. The `mpsc::Sender<DebugEvent>` is currently dropped in `VmBackendFactory::create()` (line 359 of `dap_backend.rs`). VM debug events update `DebugState` in the TEA handler but are never forwarded to the DAP adapter, so the IDE never receives `stopped`/`continued`/`thread` events.
- **Capabilities are already advertised**: `supportsConditionalBreakpoints`, `supportsLogPoints`, `supportsEvaluateForHovers` are set to `true` in the initialize response. Phase 4 implements the actual behavior behind these capabilities.
- **Protocol types exist**: `SourceBreakpoint.condition`, `.hit_condition`, `.log_message` fields are already defined in `protocol/types.rs`. The adapter logic just needs to use them.
- **Expression evaluation is partially implemented**: Basic `evaluateInFrame` and root library evaluation work. Phase 4 adds context-specific behavior (hover auto-toString, clipboard formatting).
- **Layer boundary**: `fdemon-dap` must NOT depend on `fdemon-app`. All cross-boundary communication uses trait objects, channels, or `EngineEvent` broadcasts.
