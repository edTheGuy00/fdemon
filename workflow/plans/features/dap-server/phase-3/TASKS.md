# DAP Server Phase 3: Core Debugging — Task Index

## Overview

Phase 3 builds the **adapter layer** that bridges DAP protocol messages to Dart VM Service RPCs, enabling full debugging support (breakpoints, stepping, stack traces, variables, evaluation). It also adds **stdio transport** for first-class Zed IDE and Helix editor support alongside the existing TCP transport.

**Total Tasks:** 12
**Estimated Hours:** 40-55 hours

## Task Dependency Graph

```
Wave 1 (Foundation — parallel)
┌──────────────────────────────┐     ┌──────────────────────────────┐
│  01-expand-protocol-types    │     │  02-stdio-transport          │
└──────────────┬───────────────┘     └──────────────┬───────────────┘
               │                                    │
Wave 2         │                                    │
               ▼                                    │
┌──────────────────────────────┐                    │
│  03-adapter-core-structure   │                    │
└──────────────┬───────────────┘                    │
               │                                    │
Wave 3         ├──────────────────────┐             │
               ▼                      ▼             │
┌──────────────────────────┐  ┌───────────────────────────────┐
│  04-thread-management    │  │  05-breakpoint-management     │
└──────────────┬───────────┘  └───────────────────────────────┘
               │                                    │
Wave 4         ├──────────────────────┐             │
               ▼                      ▼             │
┌──────────────────────────┐  ┌───────────────────────────────┐
│  06-execution-control    │  │  07-stack-traces-and-scopes   │
└──────────────┬───────────┘  └──────────────┬────────────────┘
               │                              │
Wave 5         │              ┌───────────────┼──────────────┐
               │              ▼               ▼              │
               │  ┌───────────────────┐  ┌──────────────────────────┐
               │  │  08-variables     │  │  09-evaluate             │
               │  └─────────┬─────────┘  └──────────────┬───────────┘
               │            │                            │
Wave 6         └────────────┼────────────────────────────┘
                            ▼
              ┌──────────────────────────────────┐
              │  10-session-integration          │
              └──────────────┬───────────────────┘
                             │
Wave 7                       ├──────────────────────────┐
                             ▼                          ▼
              ┌──────────────────────────┐  ┌──────────────────────────┐
              │  11-output-events        │  │  12-ide-config-zed-helix │◄── 02
              └──────────────────────────┘  └──────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-expand-protocol-types](tasks/01-expand-protocol-types.md) | Not Started | - | 3-4h | `fdemon-dap/src/protocol/types.rs` |
| 2 | [02-stdio-transport](tasks/02-stdio-transport.md) | Not Started | - | 3-4h | `fdemon-dap/src/transport/`, `flutter-demon/src/main.rs` |
| 3 | [03-adapter-core-structure](tasks/03-adapter-core-structure.md) | Not Started | 1 | 4-5h | `fdemon-dap/src/adapter/mod.rs` |
| 4 | [04-thread-management](tasks/04-thread-management.md) | Not Started | 3 | 3-4h | `fdemon-dap/src/adapter/threads.rs` |
| 5 | [05-breakpoint-management](tasks/05-breakpoint-management.md) | Not Started | 3 | 4-5h | `fdemon-dap/src/adapter/breakpoints.rs` |
| 6 | [06-execution-control](tasks/06-execution-control.md) | Not Started | 4 | 3-4h | `fdemon-dap/src/adapter/mod.rs` |
| 7 | [07-stack-traces-and-scopes](tasks/07-stack-traces-and-scopes.md) | Not Started | 4 | 3-4h | `fdemon-dap/src/adapter/stack.rs` |
| 8 | [08-variables](tasks/08-variables.md) | Not Started | 7 | 4-5h | `fdemon-dap/src/adapter/stack.rs` |
| 9 | [09-evaluate](tasks/09-evaluate.md) | Not Started | 7 | 2-3h | `fdemon-dap/src/adapter/evaluate.rs` |
| 10 | [10-session-integration](tasks/10-session-integration.md) | Not Started | 5, 6, 8, 9 | 5-7h | `fdemon-dap/src/server/session.rs`, `fdemon-app/src/engine.rs` |
| 11 | [11-output-events](tasks/11-output-events.md) | Not Started | 10 | 2-3h | `fdemon-dap/src/adapter/mod.rs`, `fdemon-app/src/handler/dap.rs` |
| 12 | [12-ide-config-zed-helix](tasks/12-ide-config-zed-helix.md) | Not Started | 2, 10 | 2-3h | `docs/`, `fdemon-dap/src/adapter/` |

## Success Criteria

Phase 3 is complete when:

- [ ] A DAP client (VS Code, Zed, Helix, nvim-dap) can connect to fdemon via TCP or stdio
- [ ] Full initialization handshake completes (initialize → configurationDone → attach)
- [ ] Breakpoints can be set by file URI and line number
- [ ] Exception breakpoints (All/Unhandled/None) work
- [ ] Stepping works: continue, next, stepIn, stepOut, pause
- [ ] Stack traces display correctly with source locations
- [ ] Variables can be inspected (locals, expanded objects and collections)
- [ ] Expression evaluation works in the debug console and on hover
- [ ] Log output appears in the debug console as DAP `output` events
- [ ] Zed IDE can debug a Flutter app via `.zed/debug.json` config (TCP and stdio)
- [ ] Helix can debug a Flutter app via `languages.toml` config (TCP and stdio)
- [ ] All new code has unit tests
- [ ] All existing 2,525+ tests continue to pass
- [ ] `cargo clippy --workspace` passes cleanly
- [ ] No circular dependencies introduced

## IDE Compatibility Matrix

| Feature | VS Code | Zed | Helix | nvim-dap |
|---------|---------|-----|-------|----------|
| TCP transport | Yes | Yes (`tcp_connection`) | Yes (`:debug-remote`) | Yes (`type = "server"`) |
| Stdio transport | Yes (`debugServer`) | Yes (default) | Yes (default) | Yes (default) |
| Breakpoints | Yes | Yes | Yes | Yes |
| Exception breakpoints | Yes | Yes | Yes (`<space>Ge`) | Yes |
| Stepping | Yes | Yes | Yes (`<space>Gn/i/o`) | Yes |
| Stack traces | Yes | Yes | Yes (`<space>Gsf`) | Yes |
| Variables | Yes | Yes (panel) | Yes (`<space>Gv`, flat popup) | Yes |
| Evaluate | Yes | Yes (debug console) | Yes (`:debug-eval`) | Yes |
| Hover evaluate | Yes | No (issue #32932) | No | Plugin-dependent |

## Notes

- **No circular dependencies**: `fdemon-dap` must NOT depend on `fdemon-app` or `fdemon-daemon`. The adapter receives a `VmRequestHandle` (from `fdemon-daemon`) via a trait or channel — never directly.
- **Async adapter**: The `DapAdapter` runs in a Tokio task. Communication with the DAP session uses async channels, not shared mutable state.
- **Path format**: Use filesystem paths (not `file://` URIs) in all `Source` objects — both Helix (`pathFormat: "path"`) and Zed expect this.
- **Standard events only**: Avoid proprietary event types that could crash older Helix versions (pre-24.07 strict deserialization). All custom events go in Phase 4.
- **Capabilities honesty**: Only declare capabilities that are actually implemented. Helix enables/disables UI features based on capability flags.
