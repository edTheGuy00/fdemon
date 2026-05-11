# Browser DevTools DDS Registration — Task Index

## Overview

Four-phase fix routing browser DevTools through the Flutter daemon's `devtools.serve` RPC instead of the broken `http://<DDS>/devtools/` direct URL. Begins with an external_researcher task to confirm the exact RPC contract before code changes.

**Total Tasks:** 10
**Estimated Hours:** 10-14 hours

## Task Dependency Graph

```
00-research-daemon-devtools-rpc  (external_researcher)
         │
         ▼
01-daemon-command-serve-devtools
         │
         ▼
02-daemon-message-devtools-served
         │
         ▼
03-protocol-parse-daemon-devtools-event
         │
         ▼
04-session-stores-devtools-url
         │
         ▼
05-eager-serve-on-vmservice-ready
         │
         ▼
06-open-browser-uses-served-url
         │
         ▼
07-fallback-and-recovery-toast
         │
         ▼
08-update-keybindings-doc
09-update-architecture-doc  (doc_maintainer)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 0 | [00-research-daemon-devtools-rpc](tasks/00-research-daemon-devtools-rpc.md) | Not Started | — | 1-2h | `RESEARCH.md` (output only) |
| 1 | [01-daemon-command-serve-devtools](tasks/01-daemon-command-serve-devtools.md) | Not Started | 00 | 1-2h | `daemon/commands.rs` |
| 2 | [02-daemon-message-devtools-served](tasks/02-daemon-message-devtools-served.md) | Not Started | 01 | 0.5-1h | `core/events.rs` |
| 3 | [03-protocol-parse-daemon-devtools-event](tasks/03-protocol-parse-daemon-devtools-event.md) | Not Started | 02 | 1-2h | `daemon/protocol.rs` |
| 4 | [04-session-stores-devtools-url](tasks/04-session-stores-devtools-url.md) | Not Started | 03 | 1h | `session/session.rs`, `message.rs` |
| 5 | [05-eager-serve-on-vmservice-ready](tasks/05-eager-serve-on-vmservice-ready.md) | Not Started | 04 | 1-2h | `handler/session.rs`, `actions/mod.rs` |
| 6 | [06-open-browser-uses-served-url](tasks/06-open-browser-uses-served-url.md) | Not Started | 05 | 1-2h | `handler/devtools/mod.rs` |
| 7 | [07-fallback-and-recovery-toast](tasks/07-fallback-and-recovery-toast.md) | Not Started | 06 | 1h | `handler/devtools/mod.rs`, `state.rs` (toast queue) |
| 8 | [08-update-keybindings-doc](tasks/08-update-keybindings-doc.md) | Not Started | 07 | 0.5h | `docs/KEYBINDINGS.md` |
| 9 | [09-update-architecture-doc](tasks/09-update-architecture-doc.md) | Not Started | 07 | 0.5h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 00 | `workflow/plans/bugs/browser-devtools-dds-registration/RESEARCH.md` | — |
| 01 | `crates/fdemon-daemon/src/commands.rs` | RESEARCH.md |
| 02 | `crates/fdemon-core/src/events.rs` | — |
| 03 | `crates/fdemon-daemon/src/protocol.rs` | `events.rs`, `commands.rs` |
| 04 | `crates/fdemon-app/src/session/session.rs`, `crates/fdemon-app/src/message.rs` | — |
| 05 | `crates/fdemon-app/src/handler/session.rs`, `crates/fdemon-app/src/actions/mod.rs`, `crates/fdemon-app/src/handler/update.rs` | `commands.rs` |
| 06 | `crates/fdemon-app/src/handler/devtools/mod.rs` | `session.rs` |
| 07 | `crates/fdemon-app/src/handler/devtools/mod.rs`, `crates/fdemon-app/src/state.rs` | — |
| 08 | `docs/KEYBINDINGS.md` | — |
| 09 | `docs/ARCHITECTURE.md` | — |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 06 + 07 | `handler/devtools/mod.rs` | **Sequential (same branch)** — 07 depends on 06 |
| 08 + 09 | None (KEYBINDINGS vs ARCHITECTURE) | Parallel (worktree) |
| 02 + 03 | None | Sequential by dependency (03 depends on 02) |
| All other pairs | None / sequential by dependency | Sequential per chain |

### Wave Plan

This bug fix is mostly a single dependency chain. Only the final doc tasks (08, 09) can run in parallel.

- **Wave 1**: 00 (research output).
- **Wave 2**: 01 → 02 → 03 (each depends on the previous; sequential).
- **Wave 3**: 04 → 05 → 06 → 07 (sequential chain).
- **Wave 4**: 08 + 09 in parallel.

## Success Criteria

- [ ] Pressing `B` on modern Flutter (≥ 3.16) opens DevTools successfully.
- [ ] Pressing `B` on older Flutter falls back to legacy URL with a clear recovery toast.
- [ ] `daemon.devtools.serve` failures are handled gracefully (`-32601 Method not found` → fallback).
- [ ] `daemon.devtools` event is parsed and surfaces `DaemonMessage::DevToolsServed`.
- [ ] `Session` carries the served DevTools endpoint.
- [ ] All four CI quality gates pass.
- [ ] Docs updated.

## Notes

- The exact RPC method name is verified in task 00 — current best guess `daemon.devtools.serve` (returns `{host, port}`); refine after research.
- Be mindful of `DaemonCommand` serialization shape — must match existing `commands.rs` style.
