# Widget Crash Detection - Phase 1 Task Index

## Overview

Detect Flutter framework exception blocks from stderr/stdout, buffer them into single collapsible `LogEntry` items with parsed stack traces.

**Total Tasks:** 4
**Estimated Hours:** 10-14 hours

## Task Dependency Graph

```
┌──────────────────────────────────┐
│  01-exception-block-parser       │  (fdemon-core)
│  Types + state machine + tests   │
│  Est: 4-5h                       │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│  02-session-exception-buffer     │  (fdemon-app)
│  Session integration + methods   │
│  Est: 2-3h                       │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│  03-handler-integration          │  (fdemon-app)
│  Wire stderr/stdout/exit paths   │
│  Est: 3-4h                       │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│  04-crash-entry-styling          │  (fdemon-tui, optional)
│  Visual distinction for crashes  │
│  Est: 1-2h                       │
└──────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-exception-block-parser](tasks/01-exception-block-parser.md) | Done | - | 4-5h | `fdemon-core/src/exception_block.rs` |
| 2 | [02-session-exception-buffer](tasks/02-session-exception-buffer.md) | Done | 1 | 2-3h | `fdemon-app/src/session.rs` |
| 3 | [03-handler-integration](tasks/03-handler-integration.md) | Done | 2 | 3-4h | `fdemon-app/src/handler/daemon.rs`, `handler/session.rs` |
| 4 | [04-crash-entry-styling](tasks/04-crash-entry-styling.md) | Done | 3 | 1-2h | `fdemon-core/src/exception_block.rs` |

## Success Criteria

Phase 1 is complete when:

- [x] Flutter exception blocks are detected and buffered into single LogEntry items
- [x] Stack traces within exceptions are parsed via `ParsedStackTrace::parse()`
- [x] Exception entries are collapsible in the log view
- [x] "Another exception was thrown:" one-liners produce Error-level entries
- [x] Incomplete buffers are flushed on session exit
- [x] No regression in existing log handling
- [x] All new code has unit tests
- [x] `cargo fmt && cargo check && cargo test && cargo clippy` pass

## Notes

- The existing `ParsedStackTrace`, `CollapseState`, and `LogView` collapse rendering infrastructure is reused — no changes needed for basic collapsibility
- The `ExceptionBlockParser` lives in `fdemon-core` (zero internal deps) to keep domain logic in the foundation crate
- Session-level buffering keeps exception state per-device-session, avoiding cross-session contamination
- The parser must handle exceptions arriving from any path (stderr, raw stdout, app.log events)
