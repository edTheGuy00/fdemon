# Phase 1: VM Service Debugging Foundation - Task Index

## Overview

Extend the VM Service client with all debugging RPCs, debug/isolate stream event parsing, and per-session debug state tracking. This is pure infrastructure — no user-facing changes, fully testable.

**Total Tasks:** 5
**Dispatch Waves:** 3 (see dependency graph)

## Task Dependency Graph

```
Wave 1 (parallel — no dependencies):
┌──────────────────────────────┐  ┌──────────────────────────────┐
│  01-debug-types              │  │  02-debug-stream-events      │
│  (daemon: protocol types)    │  │  (daemon: stream subscribe)  │
└─────────────┬────────────────┘  └──────────────┬───────────────┘
              │                                  │
Wave 2 (parallel — after Wave 1):                │
              │                   ┌──────────────┘
              ▼                   ▼
┌──────────────────────────────┐  ┌──────────────────────────────┐
│  03-debug-rpc-wrappers       │  │  04-session-debug-state      │
│  (daemon: debugger.rs)       │  │  (app: debug_state.rs)       │
│  depends: 01                 │  │  depends: 01                 │
└─────────────┬────────────────┘  └──────────────┬───────────────┘
              │                                  │
Wave 3:       └──────────────┬───────────────────┘
                             ▼
              ┌──────────────────────────────────┐
              │  05-message-pipeline-integration  │
              │  (app: message + handler wiring)  │
              │  depends: 02, 03, 04             │
              └──────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-debug-types](tasks/01-debug-types.md) | Done | - | `fdemon-daemon/vm_service/debugger_types.rs` |
| 2 | [02-debug-stream-events](tasks/02-debug-stream-events.md) | Done | - | `fdemon-daemon/vm_service/client.rs`, `protocol.rs` |
| 3 | [03-debug-rpc-wrappers](tasks/03-debug-rpc-wrappers.md) | Done | 1 | `fdemon-daemon/vm_service/debugger.rs`, `mod.rs` |
| 4 | [04-session-debug-state](tasks/04-session-debug-state.md) | Done | 1 | `fdemon-app/session/debug_state.rs`, `session.rs`, `handle.rs` |
| 5 | [05-message-pipeline-integration](tasks/05-message-pipeline-integration.md) | Done | 2, 3, 4 | `fdemon-app/message.rs`, `handler/mod.rs`, `handler/devtools/` |

## Success Criteria

Phase 1 is complete when:

- [ ] All 11 VM Service debugging RPCs are callable via `VmRequestHandle`
- [ ] `Debug` and `Isolate` streams are subscribed on VM Service connect/reconnect
- [ ] Debug stream events (`PauseBreakpoint`, `PauseException`, `Resume`, etc.) are parsed into typed enums
- [ ] Isolate stream events (`IsolateStart`, `IsolateRunnable`, `IsolateExit`, `IsolateReload`) are parsed
- [ ] `DebugState` tracks pause state, breakpoints, and exception mode per session
- [ ] `Message` enum has `VmServiceDebugEvent` and `VmServiceIsolateEvent` variants
- [ ] `UpdateAction` enum has debug RPC action variants
- [ ] Debug event handler in `handler/devtools/debug.rs` updates session `DebugState`
- [ ] All new code has unit tests (target: 100% on new types and RPC wrappers)
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes (no regressions)
- [ ] `cargo clippy --workspace -- -D warnings` passes

## Notes

- Phase 1 has **zero user-facing changes** — no keybindings, no TUI widgets, no CLI flags
- All new code lives in `fdemon-daemon` (RPC layer) and `fdemon-app` (state + handlers)
- The `VmRequestHandle::request()` method already supports arbitrary JSON-RPC calls — no transport changes needed
- Use `setIsolatePauseMode` (not deprecated `setExceptionPauseMode`) for exception pause configuration
- `addBreakpointWithScriptUri` is preferred over `addBreakpoint` (handles deferred libraries correctly)
- Debug RPC wrapper functions follow the existing pattern in `performance.rs` and `network.rs`: free functions taking `handle: &VmRequestHandle`
