# Phase 2: Protocol Integration (Basic Control) - Task Index

## Overview

**Goal**: Parse Flutter daemon protocol, implement hot reload/restart commands, and create a responsive development TUI with file watching.

**Duration**: 2-3 weeks

**Total Tasks**: 7

This phase transforms the basic log viewer from Phase 1 into a functional development tool with:
- Typed JSON-RPC protocol parsing
- Hot reload/restart via keyboard shortcuts
- Status bar with app state and device info
- Automatic reload on file save
- Service layer foundation for future MCP integration

## MCP Server Groundwork

Per the [MCP Server Plan](../../mcp-server/PLAN.md), Phase 2 adopts architectural patterns that enable future MCP integration:

| Pattern | Task | Purpose |
|---------|------|---------|
| **Service Layer** | 02-service-layer | Extract shared logic into traits that both TUI and future MCP can use |
| **Arc<RwLock> State** | 02-service-layer | Enable concurrent access from TUI + future MCP handlers |
| **Event Broadcasting** | 02-service-layer | Use `tokio::sync::broadcast` so multiple consumers can subscribe |
| **Command/Query Separation** | 03-command-system | Clean separation between mutations and reads |

These patterns are **not** optional enhancements—they are required architectural decisions for MCP compatibility.

---

## Task Dependency Graph

```
                         ┌──────────────────────┐
                         │  01-typed-protocol   │
                         │  (JSON-RPC structs)  │
                         └──────────┬───────────┘
                                    │
           ┌────────────────────────┼────────────────────────┐
           │                        │                        │
           ▼                        ▼                        ▼
┌──────────────────────┐  ┌─────────────────┐  ┌──────────────────────┐
│  02-service-layer    │  │  05-status-bar  │  │  07-enhanced-logging │
│  (traits, shared     │  │  (state, timer) │  │  (app.log parsing)   │
│   state, broadcast)  │  └─────────────────┘  └──────────────────────┘
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│  03-command-system   │
│  (request tracking,  │
│   response matching) │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│  04-reload-commands  │
│  ('r' reload,        │
│   'R' restart)       │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│  06-file-watcher     │
│  (auto-reload on     │
│   file save)         │
└──────────────────────┘
```

**Parallelization**: Tasks 05 and 07 can be done in parallel with the 02→03→04→06 chain.

---

## Tasks

| # | Task | Status | Depends On | Effort | Key Modules |
|---|------|--------|------------|--------|-------------|
| 1 | [01-typed-protocol](tasks/01-typed-protocol.md) | ✅ Done | - | 3-4 hrs | `daemon/protocol.rs`, `daemon/events.rs` |
| 2 | [02-service-layer](tasks/02-service-layer.md) | ✅ Done | 01 | 4-6 hrs | `services/mod.rs`, `services/*.rs` |
| 3 | [03-command-system](tasks/03-command-system.md) | ✅ Done | 02 | 3-4 hrs | `daemon/commands.rs` |
| 4 | [04-reload-commands](tasks/04-reload-commands.md) | ✅ Done | 03 | 2-3 hrs | `app/handler.rs`, `app/message.rs` |
| 5 | [05-status-bar](tasks/05-status-bar.md) | ✅ Done | 01, 02 | 2-3 hrs | `tui/widgets/status_bar.rs` |
| 6 | [06-file-watcher](tasks/06-file-watcher.md) | ✅ Done | 04 | 3-4 hrs | `watcher/mod.rs`, `Cargo.toml` |
| 7 | [07-enhanced-logging](tasks/07-enhanced-logging.md) | ✅ Done | 01 | 2-3 hrs | `app/handler.rs`, `core/types.rs` |

**Total Estimated Effort**: 20-27 hours

---

## Task Summaries

| Task | Description |
|------|-------------|
| **01-typed-protocol** | Define typed Rust structs for all Flutter daemon JSON-RPC events (`app.log`, `app.start`, `app.progress`, etc.) with serde deserialization |
| **02-service-layer** | Create service traits (`FlutterController`, `LogService`, `StateService`) with `Arc<RwLock>` shared state and `broadcast` channels for MCP compatibility |
| **03-command-system** | Implement request ID tracking, response matching with timeout, and `send_command()` abstraction for daemon communication |
| **04-reload-commands** | Wire 'r' key to hot reload (`app.restart`) and 'R' key to hot restart (`app.restart { fullRestart: true }`) |
| **05-status-bar** | Create status bar widget showing app state (●/○/↻), device name, platform, session timer, and last reload time |
| **06-file-watcher** | Integrate `notify-debouncer-full` to watch `lib/` folder with 500ms debounce, triggering auto-reload on file save |
| **07-enhanced-logging** | Parse `app.log` events to extract Flutter print() output, color-code errors/warnings, handle `daemon.logMessage` |

---

## New Dependencies Required

```toml
[dependencies]
# File watching (Task 06)
notify = "7"
notify-debouncer-full = "0.4"
```

---

## New Module Structure

After Phase 2, the `src/` directory will include:

```
src/
├── services/                    # NEW - Service Layer (Task 02)
│   ├── mod.rs
│   ├── flutter_controller.rs    # FlutterController trait + impl
│   ├── log_service.rs           # LogService trait + impl
│   └── state_service.rs         # StateService trait + SharedState
│
├── daemon/
│   ├── mod.rs
│   ├── process.rs               # Existing
│   ├── protocol.rs              # Enhanced (Task 01)
│   ├── events.rs                # NEW - Typed event structs (Task 01)
│   └── commands.rs              # NEW - Command/response system (Task 03)
│
├── watcher/                     # NEW - File Watcher (Task 06)
│   └── mod.rs
│
├── tui/
│   └── widgets/
│       ├── log_view.rs          # Existing
│       └── status_bar.rs        # NEW (Task 05)
│
└── ... (existing modules)
```

---

## Flutter Daemon Events Reference

Events that need typed structs in Task 01:

| Event | Description | Key Fields |
|-------|-------------|------------|
| `daemon.connected` | Initial connection established | `version`, `pid` |
| `daemon.logMessage` | Daemon-level log messages | `level`, `message` |
| `app.start` | App started/attached | `appId`, `deviceId`, `mode` |
| `app.started` | App fully launched | `appId` |
| `app.log` | Flutter print() output | `log`, `error`, `stackTrace` |
| `app.progress` | Build/operation progress | `appId`, `id`, `progressId`, `message`, `finished` |
| `app.stop` | App stopped | `appId` |
| `app.debugPort` | DevTools debug port | `appId`, `port`, `wsUri` |

---

## Success Criteria

Phase 2 is complete when:

- [x] All daemon events are parsed into typed Rust structs
- [x] Service layer traits are defined and implemented
- [x] Shared state uses `Arc<RwLock>` pattern
- [x] Event broadcasting uses `tokio::sync::broadcast`
- [x] Commands have request ID tracking and response matching
- [x] 'r' key triggers hot reload with visual feedback
- [x] 'R' key triggers hot restart with visual feedback
- [x] Status bar shows: state indicator, device name, platform, session timer
- [x] File watcher detects changes in `lib/` folder
- [x] File changes trigger auto-reload with 500ms debounce
- [x] Flutter print() output displays with proper formatting
- [x] Error messages are colored red
- [x] All new code has unit tests
- [x] `cargo test` passes
- [x] `cargo clippy` has no warnings

---

## Milestone Deliverable

A functional development TUI with:
- Hot reload via keyboard or automatic file watching
- Status display showing app state and device
- Properly formatted log output with colors
- Architecture ready for future MCP server integration

---

## Edge Cases & Risks

| Risk | Mitigation |
|------|------------|
| Response timeout for reload command | 30s timeout, show "Reload timed out" message |
| Multiple rapid file changes | Debounce with 500ms delay, coalesce changes |
| Daemon crashes during reload | Detect exit event, show error, offer restart |
| Invalid JSON from daemon | Graceful fallback to raw message display |
| File watcher permission errors | Log warning, continue without watching |
| Concurrent state access | Arc<RwLock> with proper lock ordering |
| Broadcast channel full | Use bounded channel with oldest-drop policy |

---

## Notes

- Task 02 (service layer) is the most critical for MCP compatibility
- File watcher paths should be configurable (via `fdemon.toml` in Phase 3)
- Status bar timer should pause when app is stopped
- Consider adding 's' key binding for stop (deferred to Phase 3)
- The broadcast channel setup in Task 02 enables future MCP event subscription