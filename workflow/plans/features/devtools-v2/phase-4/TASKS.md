# Phase 4: Network Monitor Tab - Task Index

## Overview

Add a new Network tab to DevTools showing HTTP/HTTPS/WebSocket traffic with request/response inspection. Uses the `ext.dart.io.*` VM Service extensions to poll HTTP profile data, display a scrollable request table, and provide detailed request/response inspection with headers, bodies, and timing.

**Total Tasks:** 9
**Waves:** 5 (01 solo, then 02+03 parallel, then 04+06+07 parallel, then 05+08 parallel, then 09 solo)

## Task Dependency Graph

```
Wave 1
┌───────────────────────────────────────┐
│ 01-add-network-domain-types           │
│ (fdemon-core)                         │
└──────────────────┬────────────────────┘
                   │
Wave 2 (parallel — different crates)
        ┌──────────┴──────────────────────────────┐
        ▼                                         ▼
┌──────────────────────────────────┐  ┌───────────────────────────────────────┐
│ 02-vm-service-network-extensions │  │ 03-add-network-state-and-messages     │
│ (fdemon-daemon)                  │  │ (fdemon-app)                          │
│ depends: 01                      │  │ depends: 01                           │
└──────────────┬───────────────────┘  └───────────────┬─────────────────────┬─┘
               │                                      │                     │
Wave 3 (parallel — different files/crates)            │                     │
               │                 ┌────────────────────┤                     │
               │                 ▼                    ▼                     ▼
               │   ┌──────────────────────────┐ ┌──────────────────────────────┐
               │   │ 06-request-table-widget  │ │ 07-request-details-widget    │
               │   │ (fdemon-tui, new file)   │ │ (fdemon-tui, new file)       │
               │   │ depends: 01, 03          │ │ depends: 01, 03              │
               │   └──────────┬───────────────┘ └──────────────┬───────────────┘
               │              │                                │
               │   ┌──────────────────────────────────────┐    │
               │   │ 04-network-handlers-and-keybindings  │    │
               │   │ (fdemon-app handler/)                 │    │
               │   │ depends: 03                           │    │
               │   └──────────────┬───────────────────────┘    │
               │                  │                            │
Wave 4 (parallel — different crates/files)                     │
               │                  │                            │
        ┌──────┘                  │          ┌─────────────────┘
        ▼                         ▼          ▼
┌───────────────────────────────────┐ ┌───────────────────────────────┐
│ 05-wire-network-monitoring-engine │ │ 08-wire-network-monitor-panel │
│ (fdemon-app engine/actions)       │ │ (fdemon-tui devtools/mod.rs)  │
│ depends: 02, 04                   │ │ depends: 04, 06, 07           │
└──────────────┬────────────────────┘ └──────────────┬────────────────┘
               │                                     │
Wave 5         └────────────────┬────────────────────┘
                                ▼
                 ┌─────────────────────────────────┐
                 │ 09-final-test-and-cleanup        │
                 │ (workspace-wide)                 │
                 │ depends: 05, 08                  │
                 └─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Crate | Modules |
|---|------|--------|------------|-------|---------|
| 1 | [01-add-network-domain-types](tasks/01-add-network-domain-types.md) | Done | - | `fdemon-core` | `network.rs` (NEW), `lib.rs` |
| 2 | [02-implement-vm-service-network-extensions](tasks/02-implement-vm-service-network-extensions.md) | Done | 1 | `fdemon-daemon` | `vm_service/network.rs` (NEW), `vm_service/extensions/mod.rs`, `vm_service/mod.rs` |
| 3 | [03-add-network-state-and-messages](tasks/03-add-network-state-and-messages.md) | Done | 1 | `fdemon-app` | `session/network.rs` (NEW), `session/session.rs`, `session/mod.rs`, `state.rs`, `message.rs`, `handler/mod.rs` |
| 4 | [04-implement-network-handlers-and-keybindings](tasks/04-implement-network-handlers-and-keybindings.md) | Done | 3 | `fdemon-app` | `handler/devtools/network.rs` (NEW), `handler/devtools/mod.rs`, `handler/keys.rs`, `handler/update.rs` |
| 5 | [05-wire-network-monitoring-engine](tasks/05-wire-network-monitoring-engine.md) | Done | 2, 4 | `fdemon-app` | `actions.rs`, `process.rs`, `session/handle.rs` |
| 6 | [06-implement-request-table-widget](tasks/06-implement-request-table-widget.md) | Done | 1, 3 | `fdemon-tui` | `widgets/devtools/network/request_table.rs` (NEW) |
| 7 | [07-implement-request-details-widget](tasks/07-implement-request-details-widget.md) | Done | 1, 3 | `fdemon-tui` | `widgets/devtools/network/request_details.rs` (NEW) |
| 8 | [08-wire-network-monitor-panel](tasks/08-wire-network-monitor-panel.md) | Done | 4, 6, 7 | `fdemon-tui` | `widgets/devtools/network/mod.rs` (NEW), `widgets/devtools/mod.rs` |
| 9 | [09-final-test-and-cleanup](tasks/09-final-test-and-cleanup.md) | Done | 5, 8 | workspace | All devtools modules |

## Dispatch Plan

**Wave 1** (solo — foundation types):
- Task 01: Add network domain types (fdemon-core only)

**Wave 2** (parallel — different crates):
- Task 02: Implement VM Service network extensions (fdemon-daemon)
- Task 03: Add network state and messages (fdemon-app state/message layer)

**Wave 3** (parallel — different files/crates, no conflicts):
- Task 04: Implement network handlers and key bindings (fdemon-app handler/ — new file)
- Task 06: Implement request table widget (fdemon-tui — new file)
- Task 07: Implement request details widget (fdemon-tui — new file)

**Wave 4** (parallel — different crates/files):
- Task 05: Wire network monitoring into engine/actions (fdemon-app engine layer)
- Task 08: Wire network monitor panel and tab bar (fdemon-tui devtools integration)

**Wave 5** (solo — final verification):
- Task 09: Full test and cleanup pass

## Success Criteria

Phase 4 is complete when:

- [ ] Network tab accessible via `'n'` key in DevTools mode
- [ ] HTTP requests displayed in scrollable table (method, URI, status, duration, size)
- [ ] Selecting a request shows detailed info (headers, body, timing)
- [ ] Recording can be toggled on/off
- [ ] Request history can be cleared
- [ ] Filter by method, status, or free text
- [ ] Pending requests shown with indicator
- [ ] WebSocket entries shown when available
- [ ] VM Service `ext.dart.io.*` extensions properly called
- [ ] Responsive layout (wide: table + details side-by-side; narrow: stacked)
- [ ] All new code has unit tests (30+ new tests)
- [ ] All existing tests pass (no regressions)
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy` clean

## Keyboard Shortcuts (New in Phase 4)

| Key | Action | Context |
|-----|--------|---------|
| `n` | Switch to Network panel | DevTools mode |
| `Up/Down/j/k` | Navigate request list | Network panel active |
| `Enter` | View request details (narrow mode) | Network panel, request selected |
| `Esc` | Close details / deselect | Network panel |
| `g/h/q/s/t` | Switch detail sub-tabs (General/Headers/reQuest/reSponse/Timing) | Network panel, detail view |
| `Space` | Toggle recording on/off | Network panel active |
| `Ctrl+x` | Clear recorded requests | Network panel active |
| `/` | Enter filter mode | Network panel active |

## Notes

- **Phases 1-3 assumed complete**: Tasks reference the post-Phase-3 state where the handler is split into `handler/devtools/{mod,inspector,performance}.rs` and the performance tab has frame chart + memory chart.
- **`ext.dart.io.*` protocol version 4.0**: All implementations target Dart 3.0+ / Flutter 3.10+. IDs are `String`, timestamps are microseconds since Unix epoch (`i64`), bodies are `Vec<u8>` from JSON `int[]` arrays.
- **HTTP profiling must be explicitly enabled**: `ext.dart.io.httpEnableTimelineLogging` must be called with `enabled: true` before `getHttpProfile` returns data. This is done automatically when the Network tab is activated or recording starts.
- **Bodies are NOT in list responses**: `getHttpProfile` returns request summaries without bodies. Bodies require a separate `getHttpProfileRequest(id)` call — fetched on-demand when the user selects a request and views the Request/Response body tabs.
- **Incremental polling**: Use the `timestamp` from the last `HttpProfile` response as the `updatedSince` parameter for the next poll. This avoids re-fetching all requests each poll cycle.
- **Socket profiling is optional**: `getSocketProfile` may not be available on all Dart versions. The implementation is defensive — socket entries are shown when available, omitted otherwise.
- **Release mode limitation**: `ext.dart.io.*` extensions only exist in debug and profile build modes. The Network tab shows an informative "not available in release mode" message when extensions are not registered.
