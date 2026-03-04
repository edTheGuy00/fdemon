# Phase 2: DAP Protocol & Server Infrastructure - Task Index

## Overview

Implement the DAP wire protocol, TCP server, and client session management within a new `fdemon-dap` crate. Integrate the DAP server as a toggleable Engine service with configuration, CLI flags, keybinding, and status bar display. By the end of this phase, a DAP client can connect to fdemon, complete the initialization handshake, and receive a capabilities response — but no debugging features yet.

**Total Tasks:** 7
**Dispatch Waves:** 4 (see dependency graph)

## Task Dependency Graph

```
Wave 1 (parallel — no dependencies):
┌──────────────────────────────────┐  ┌──────────────────────────────────┐
│  01-dap-crate-and-protocol       │  │  02-dap-settings                 │
│  (fdemon-dap: types + codec)     │  │  (fdemon-app: config + settings) │
└─────────────┬────────────────────┘  └──────────────┬───────────────────┘
              │                                      │
Wave 2 (parallel — after Wave 1):                    │
              │                       ┌──────────────┘
              ▼                       ▼
┌──────────────────────────────────┐  ┌──────────────────────────────────┐
│  04-tcp-server-and-session       │  │  03-dap-messages-and-handler     │
│  (fdemon-dap: server + session)  │  │  (fdemon-app: messages + state)  │
│  depends: 01                     │  │  depends: 02                     │
└─────────────┬────────────────────┘  └────────┬─────────┬───────────────┘
              │                                │         │
Wave 3 (parallel — after Wave 2):              │         │
              │                   ┌────────────┘         │
              ▼                   ▼                      │
┌──────────────────────────────────┐  ┌──────────────────┴───────────────┐
│  05-dap-service-and-cli          │  │  06-status-bar-and-header        │
│  (fdemon-dap + binary: service)  │  │  (fdemon-tui: badges + hints)   │
│  depends: 03, 04                 │  │  depends: 03                     │
└─────────────┬────────────────────┘  └──────────────┬───────────────────┘
              │                                      │
Wave 4:       └──────────────┬───────────────────────┘
                             ▼
              ┌──────────────────────────────────┐
              │  07-auto-start-and-keybinding     │
              │  (fdemon-app + binary: toggle)    │
              │  depends: 05, 06                  │
              └──────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-dap-crate-and-protocol](tasks/01-dap-crate-and-protocol.md) | Not Started | - | `fdemon-dap/` (new crate), `Cargo.toml` (workspace) |
| 2 | [02-dap-settings](tasks/02-dap-settings.md) | Not Started | - | `fdemon-app/config/types.rs`, `settings_items.rs`, `handler/settings.rs` |
| 3 | [03-dap-messages-and-handler](tasks/03-dap-messages-and-handler.md) | Not Started | 2 | `fdemon-app/message.rs`, `state.rs`, `handler/dap.rs` (new) |
| 4 | [04-tcp-server-and-session](tasks/04-tcp-server-and-session.md) | Not Started | 1 | `fdemon-dap/server/mod.rs`, `server/session.rs` |
| 5 | [05-dap-service-and-cli](tasks/05-dap-service-and-cli.md) | Not Started | 3, 4 | `fdemon-dap/service.rs`, `src/main.rs`, `engine.rs`, runners |
| 6 | [06-status-bar-and-header](tasks/06-status-bar-and-header.md) | Not Started | 3 | `fdemon-tui/widgets/log_view/mod.rs`, `widgets/header.rs`, `render/mod.rs` |
| 7 | [07-auto-start-and-keybinding](tasks/07-auto-start-and-keybinding.md) | Not Started | 5, 6 | `fdemon-app/handler/keys.rs`, `handler/dap.rs`, `config/settings.rs` |

## Success Criteria

Phase 2 is complete when:

- [ ] `fdemon-dap` crate compiles and passes tests
- [ ] DAP protocol codec handles Content-Length framing correctly (partial reads, zero-length, oversized)
- [ ] DAP message types (Request, Response, Event) serialize/deserialize correctly
- [ ] TCP server accepts connections and completes DAP initialization handshake
- [ ] Client session state machine handles initialize → configurationDone → attach → disconnect flow
- [ ] `DapSettings` struct with `enabled`, `auto_start_in_ide`, `port`, `bind_address` in config
- [ ] DAP settings appear in settings panel (`,` → DAP Server section)
- [ ] `fdemon --dap-port PORT` CLI flag starts DAP server on a fixed port
- [ ] Smart auto-start works: running inside VS Code terminal auto-starts DAP (zero config, `auto_start_in_ide = true` by default)
- [ ] `D` keybinding toggles DAP server on/off in Normal mode
- [ ] Status bar shows `[DAP :PORT]` badge when server is running
- [ ] `cargo test -p fdemon-dap` passes
- [ ] `cargo test --workspace` passes (no regressions)
- [ ] `cargo clippy --workspace -- -D warnings` clean

## Notes

- Phase 2 creates the `fdemon-dap` crate — a new workspace member under `crates/`
- The `crates/*` glob in root `Cargo.toml` auto-includes it as a workspace member
- DAP protocol types are hand-rolled (referencing the DAP spec and `dapts` crate for correctness), not imported from an external crate
- The DAP server is NOT a separate runner mode — it's a service within TUI/headless mode
- Phase 2 implements the transport and handshake only — no debugging features (those are Phase 3)
- The existing Phase 1 debug stubs in `handler/devtools/debug.rs` and `actions/mod.rs` remain as-is for Phase 3
