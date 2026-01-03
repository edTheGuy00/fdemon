# Phase 1: Foundation (Proof of Concept) - Task Index

## Overview

**Goal**: Prove we can spawn Flutter, communicate via JSON-RPC, and display output in a TUI.

**Duration**: 1-2 weeks

**Total Tasks**: 6

This phase establishes the foundational infrastructure for Flutter Demon using **Clean Architecture** principles and **The Elm Architecture (TEA)** pattern for state management. By the end, we will have a working TUI that spawns a Flutter process, displays formatted log output, and exits cleanly.

### Architecture Highlights

- **Library + Binary Split**: Core logic in `lib.rs`, thin entry in `main.rs`
- **Layered Structure**: core/ → app/ → tui/ → daemon/ → common/
- **TEA Pattern**: Model (AppState) + Update (handler) + View (render)
- **Trait-based Abstractions**: Prepared for future extensibility

---

## Task Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   ┌──────────────────┐                                                      │
│   │  01-project-init │                                                      │
│   │  (Cargo.toml,    │                                                      │
│   │   module stubs)  │                                                      │
│   └────────┬─────────┘                                                      │
│            │                                                                │
│            ▼                                                                │
│   ┌──────────────────┐     ┌──────────────────┐                            │
│   │  02-error-setup  │     │  03-tui-shell    │                            │
│   │  (color-eyre,    │     │  (ratatui init,  │                            │
│   │   tracing)       │     │   basic layout)  │                            │
│   └────────┬─────────┘     └────────┬─────────┘                            │
│            │                        │                                       │
│            └───────────┬────────────┘                                       │
│                        │                                                    │
│                        ▼                                                    │
│            ┌──────────────────┐                                            │
│            │  04-flutter-spawn │                                            │
│            │  (tokio process,  │                                            │
│            │   stdin/stdout)   │                                            │
│            └────────┬─────────┘                                            │
│                     │                                                       │
│                     ▼                                                       │
│            ┌──────────────────┐                                            │
│            │  05-output-display│                                            │
│            │  (read stdout,    │                                            │
│            │   show in TUI)    │                                            │
│            └────────┬─────────┘                                            │
│                     │                                                       │
│                     ▼                                                       │
│            ┌──────────────────┐                                            │
│            │  06-graceful-exit │                                            │
│            │  (signal handling,│                                            │
│            │   process cleanup)│                                            │
│            └──────────────────┘                                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Tasks

| # | Task | Status | Depends On | Effort | Key Modules |
|---|------|--------|------------|--------|-------------|
| 1 | [01-project-init](tasks/01-project-init.md) | ✅ Done | - | 3-4 hrs | `lib.rs`, `main.rs`, `core/`, `app/`, `tui/`, `daemon/`, `common/` |
| 2 | [02-error-setup](tasks/02-error-setup.md) | ✅ Done | 01 | 2-3 hrs | `common/error.rs`, `core/types.rs` (add chrono) |
| 3 | [03-tui-shell](tasks/03-tui-shell.md) | ✅ Done | 01 | 3-4 hrs | `tui/widgets/log_view.rs`, `app/state.rs`, `app/handler.rs` |
| 4 | [04-flutter-spawn](tasks/04-flutter-spawn.md) | ✅ Done | 02, 03 | 4-6 hrs | `daemon/process.rs`, `daemon/protocol.rs`, `tui/mod.rs` |
| 5 | [05-output-display](tasks/05-output-display.md) | ✅ Done | 04 | 3-4 hrs | `tui/widgets/log_view.rs`, `core/types.rs` |
| 6 | [06-graceful-exit](tasks/06-graceful-exit.md) | ✅ Done | 05 | 2-3 hrs | `common/signals.rs`, `tui/mod.rs`, `app/mod.rs` |

**Total Estimated Effort**: 17-24 hours

### Task Summaries

| Task | Description |
|------|-------------|
| **01-project-init** | Set up Clean Architecture structure with TEA pattern, all module stubs |
| **02-error-setup** | Add chrono for timestamps, expand error types, enhance logging |
| **03-tui-shell** | Implement scroll state management, enhance widgets with TEA integration |
| **04-flutter-spawn** | Bridge async Flutter process to TEA message system via channels |
| **05-output-display** | Rich LogEntry formatting with colors, timestamps, source indicators |
| **06-graceful-exit** | Signal handling, graceful shutdown, process cleanup verification |

---

## Success Criteria

Phase 1 is complete when:

- [x] `cargo build` compiles library and binary without errors
- [x] `cargo test` runs all unit tests successfully
- [x] `cargo run -- /path/to/project` launches TUI
- [x] TUI displays header with shortcuts, bordered log area, status bar
- [x] Flutter process spawns and output appears as formatted log entries
- [x] Log entries show timestamps, colored levels (ERR/WRN/INF), source prefixes
- [x] Scrolling works (j/k/arrows, Page Up/Down, g/G)
- [x] Auto-scroll follows new content, disables on manual scroll
- [x] Pressing 'q', Escape, or Ctrl+C triggers graceful shutdown
- [x] SIGINT/SIGTERM signals trigger shutdown via signal handler
- [x] Flutter receives `daemon.shutdown` before force-kill
- [x] No orphaned Flutter processes after exit (`ps aux | grep flutter`)
- [x] Terminal fully restored (cursor visible, echo enabled)
- [x] Tracing logs written to `~/.local/share/flutter-demon/logs/`

---

## Notes

- Tasks 02 and 03 can be worked on in parallel after 01 is complete
- Task 01 is foundational - establishes the complete architecture
- Task 04 is the critical path - bridges async/sync and integrates with TEA
- Testing requires a valid Flutter project; use `flutter create /tmp/test_project`
- All tasks include unit tests where applicable
- Architecture follows Clean Architecture + TEA for maintainability

## Module Dependency Rules

| Layer | Can Import From |
|-------|-----------------|
| `main.rs` | `lib.rs` public API only |
| `tui/` | `app/`, `core/`, `common/` |
| `app/` | `core/`, `daemon/`, `common/` |
| `daemon/` | `core/`, `common/` |
| `core/` | `common/` only |
| `common/` | External crates only |