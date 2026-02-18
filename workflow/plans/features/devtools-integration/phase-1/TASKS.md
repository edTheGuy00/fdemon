# Phase 1: VM Service Client Foundation + Structured Errors + Hybrid Logging — Task Index

## Overview

Establish automatic WebSocket connection to the Dart VM Service on `app.debugPort`, solve the widget crash log invisibility problem by subscribing to `Flutter.Error` Extension events, and add hybrid logging via the `Logging` stream. No keybinding changes — the connection is invisible to the user.

**Total Tasks:** 8
**Estimated Hours:** 28-40 hours

## Task Dependency Graph

```
┌─────────────────────┐     ┌─────────────────────┐
│ 01-websocket-deps   │     │ 02-capture-ws-uri   │
└─────────┬───────────┘     └──────────┬──────────┘
          │                            │
          ▼                            │
┌─────────────────────┐                │
│ 03-vm-protocol      │                │
└─────────┬───────────┘                │
          │                            │
          ▼                            │
┌─────────────────────┐                │
│ 04-vm-client        │                │
└─────────┬───────────┘                │
          │                            │
          ▼                            │
┌─────────────────────┐                │
│ 05-vm-introspection │                │
└────┬────────────┬───┘                │
     │            │                    │
     ▼            ▼                    │
┌──────────┐ ┌──────────┐             │
│ 06-struc │ │ 07-log   │◄────────────┘
│  errors  │ │  stream  │
└────┬─────┘ └────┬─────┘
     │            │
     └──────┬─────┘
            ▼
  ┌─────────────────────┐
  │ 08-session-         │
  │  integration        │
  └─────────────────────┘
```

## Waves (Parallelizable Groups)

| Wave | Tasks | Description |
|------|-------|-------------|
| **1** | 01, 02 | Prerequisites: deps + ws_uri capture (independent, parallel) |
| **2** | 03 | Protocol types (depends on 01) |
| **3** | 04 | WebSocket client (depends on 03) |
| **4** | 05 | VM introspection methods (depends on 04) |
| **5** | 06, 07 | Error + logging streams (parallel, depend on 02 + 05) |
| **6** | 08 | Session integration (depends on 06 + 07) |

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate | Key Modules |
|---|------|--------|------------|------------|-------|-------------|
| 1 | [01-websocket-deps](tasks/01-websocket-deps.md) | Done | - | 1h | workspace, fdemon-daemon | `Cargo.toml` |
| 2 | [02-capture-ws-uri](tasks/02-capture-ws-uri.md) | Done | - | 2-3h | fdemon-core, fdemon-app | `session.rs`, `handler/session.rs`, `engine.rs`, `types.rs` |
| 3 | [03-vm-protocol](tasks/03-vm-protocol.md) | Done | 01 | 4-5h | fdemon-daemon | `vm_service/protocol.rs` |
| 4 | [04-vm-client](tasks/04-vm-client.md) | Done | 01, 03 | 5-7h | fdemon-daemon | `vm_service/client.rs` |
| 5 | [05-vm-introspection](tasks/05-vm-introspection.md) | Done | 04 | 3-4h | fdemon-daemon | `vm_service/client.rs` |
| 6 | [06-structured-errors](tasks/06-structured-errors.md) | Done | 02, 05 | 4-5h | fdemon-daemon | `vm_service/errors.rs` |
| 7 | [07-logging-stream](tasks/07-logging-stream.md) | Done | 02, 05 | 3-4h | fdemon-daemon | `vm_service/logging.rs` |
| 8 | [08-session-integration](tasks/08-session-integration.md) | Done | 06, 07 | 6-8h | fdemon-app, fdemon-tui | `session.rs`, `engine.rs`, `message.rs`, `status_bar.rs` |

## Success Criteria

Phase 1 is complete when:

- [ ] `ws_uri` captured from `app.debugPort` and stored in `Session`
- [ ] `SharedState.devtools_uri` populated (no longer hardcoded `None`)
- [ ] WebSocket connection **auto-established** on `app.debugPort` (no user action)
- [ ] `getVM` and `getIsolate` calls return valid data
- [ ] Connection handles gracefully with session lifecycle
- [ ] Reconnection works after brief disconnects
- [ ] Status bar shows `[VM]` indicator when connected
- [ ] **Extension stream subscribed and `Flutter.Error` events captured**
- [ ] **Widget crash logs are now visible as collapsible error entries**
- [ ] **Structured error JSON parsed into LogEntry with stack trace**
- [ ] **Logging stream subscribed and receiving events**
- [ ] **VM LogRecords converted to LogEntry with correct level**
- [ ] **VM logs merged with daemon logs in unified list**
- [ ] **Apps using `dart:developer log()` show accurate log levels**
- [ ] **Apps using Logger/Talker still work via daemon fallback**
- [ ] **Graceful fallback to ExceptionBlockParser when VM Service unavailable**
- [ ] All new code has unit tests
- [ ] No regressions in existing functionality
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes

## New Module Structure

Phase 1 creates a new `vm_service/` module in `fdemon-daemon`:

```
crates/fdemon-daemon/src/
├── vm_service/
│   ├── mod.rs          # Module exports, VmServiceHandle type
│   ├── protocol.rs     # JSON-RPC types, request/response, event parsing
│   ├── client.rs       # VmServiceClient: WebSocket connect/disconnect/reconnect
│   ├── errors.rs       # Flutter.Error Extension event parsing → LogEntry
│   └── logging.rs      # Logging stream LogRecord parsing → LogEntry
└── lib.rs              # Add `pub mod vm_service;`
```

## Notes

- **No keybinding changes** in Phase 1 — VM Service connection is invisible to the user
- The `vm_service/` module lives in `fdemon-daemon` (same layer as Flutter process I/O)
- VM Service logs are sent to `fdemon-app` via the existing `Message` channel
- The `ExceptionBlockParser` remains as fallback — it's not removed or modified
- Files > 500 lines should be split (per CODE_STANDARDS.md)
