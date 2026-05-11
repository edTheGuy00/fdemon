# Phase 2 — Handlers — Task Index

## Overview

Wire the Phase 1 messages into key bindings, `update()` routing, and per-section handlers. After this phase, key presses mutate state; widgets still ignore the new state (Phase 3 wires rendering).

**Total Tasks:** 2
**Estimated Hours:** 6-8 hours

## Task Dependency Graph

```
03-perf-keyboard-handlers       04-perf-mouse-handlers
        (parallel)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 3 | [03-perf-keyboard-handlers](tasks/03-perf-keyboard-handlers.md) | Not Started | Phase 1 | 4-5h | `handler/keys.rs`, `handler/devtools/performance.rs`, `handler/update.rs` |
| 4 | [04-perf-mouse-handlers](tasks/04-perf-mouse-handlers.md) | Not Started | Phase 1 | 2-3h | `handler/devtools/performance.rs`, `handler/update.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read |
|------|----------------------|-----------|
| 03 | `handler/keys.rs`, `handler/devtools/performance.rs`, `handler/update.rs` | `session/performance.rs`, `message.rs` |
| 04 | `handler/devtools/performance.rs`, `handler/update.rs` | `session/performance.rs`, `message.rs` |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 03 + 04 | `handler/devtools/performance.rs`, `handler/update.rs` | **Sequential (same branch)** — both add handlers and routing |

### Wave Plan

Run 03 first (keyboard); then 04 (mouse). Mouse handlers reuse the focus/scroll handler functions added in 03.

## Success Criteria

- [ ] Pressing `Tab`/`Shift+Tab` cycles `focused_section`.
- [ ] Pressing `j/k`/arrows updates the focused section's scroll offset or row selection.
- [ ] Pressing `Home`/`End` jumps to extremes.
- [ ] Mouse clicks on section areas focus the section.
- [ ] All new `Message` variants are routed in `update.rs`.
- [ ] Unit tests cover handler logic for each section + bounds.
- [ ] `cargo test --workspace` and `cargo clippy -- -D warnings` pass.
