# Phase 4: Public API Surface and Visibility - Task Index

## Overview

Phase 4 defines clean, documented public APIs for each crate, locks down internal implementation details with `pub(crate)` visibility, and adds an `EnginePlugin` trait for pro repo extensibility. This phase is purely about API hygiene and extension points -- no structural file moves or new features.

**Total Tasks:** 7
**Estimated Hours:** 18-26 hours

## Task Dependency Graph

```
Wave 1 (parallel - per-crate visibility lockdown):
┌──────────────────────────────┐  ┌──────────────────────────────┐
│  01-lock-down-fdemon-core    │  │  02-lock-down-fdemon-daemon  │
│  (pub(crate) + clean API)    │  │  (pub(crate) + clean API)    │
└──────────────┬───────────────┘  └──────────────┬───────────────┘
               │                                 │
               └────────────┬────────────────────┘
                            ▼
Wave 2 (depends on core + daemon APIs being stable):
┌──────────────────────────────┐  ┌──────────────────────────────┐
│  03-lock-down-fdemon-app     │  │  04-lock-down-fdemon-tui     │
│  (pub(crate) + clean API)    │  │  (pub(crate) + clean API)    │
└──────────────┬───────────────┘  └──────────────────────────────┘
               │
               ▼
Wave 3 (depends on app API being final):
┌──────────────────────────────┐
│  05-engine-plugin-trait      │
│  (EnginePlugin + register)   │
└──────────────┬───────────────┘
               │
               ▼
Wave 4 (depends on plugin trait):
┌──────────────────────────────┐
│  06-crate-level-docs         │
│  (//! docs for all crates)   │
└──────────────┬───────────────┘
               │
               ▼
Wave 5 (final):
┌──────────────────────────────┐
│  07-verify-and-document      │
│  (extension API doc + verify)│
└──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Key Modules |
|---|------|--------|------------|------------|-------------|
| 1 | [01-lock-down-fdemon-core](tasks/01-lock-down-fdemon-core.md) | [x] Done | - | 2-3h | `crates/fdemon-core/src/` |
| 2 | [02-lock-down-fdemon-daemon](tasks/02-lock-down-fdemon-daemon.md) | [x] Done | - | 2-3h | `crates/fdemon-daemon/src/` |
| 3 | [03-lock-down-fdemon-app](tasks/03-lock-down-fdemon-app.md) | [x] Done | 1, 2 | 4-6h | `crates/fdemon-app/src/` |
| 4 | [04-lock-down-fdemon-tui](tasks/04-lock-down-fdemon-tui.md) | [x] Done | - | 2-3h | `crates/fdemon-tui/src/` |
| 5 | [05-engine-plugin-trait](tasks/05-engine-plugin-trait.md) | [x] Done | 3 | 3-4h | `crates/fdemon-app/src/engine.rs`, `crates/fdemon-app/src/plugin.rs` |
| 6 | [06-crate-level-docs](tasks/06-crate-level-docs.md) | [x] Done | 1, 2, 3, 4, 5 | 2-3h | All `lib.rs` files |
| 7 | [07-verify-and-document](tasks/07-verify-and-document.md) | [x] Done | 6 | 3-4h | `docs/EXTENSION_API.md`, `docs/ARCHITECTURE.md` |

## Success Criteria

Phase 4 is complete when:

- [ ] Each crate has internal helpers marked `pub(crate)` (not leaking through public API)
- [ ] `fdemon-core` exports only domain types, error types, discovery functions, and prelude
- [ ] `fdemon-daemon` does not export `RawMessage`, `strip_brackets()`, `LogEntryInfo`, or `next_request_id()`
- [ ] `fdemon-app` handler submodules are `pub(crate)` (only `update()`, `UpdateAction`, `Task`, `UpdateResult` exported)
- [ ] `fdemon-app` Engine struct fields are private (accessed via methods)
- [ ] `fdemon-tui` internal modules (`event`, `layout`, `terminal`, `startup`) are `pub(crate)`
- [ ] `EnginePlugin` trait exists with `on_start`, `on_message`, `on_shutdown` methods
- [ ] `Engine::register_plugin()` and `Engine::unregister_plugin()` methods exist
- [ ] Each crate has `//!` crate-level documentation describing purpose and public API
- [ ] `docs/EXTENSION_API.md` exists documenting how pro features hook in
- [ ] An example plugin compiles against the API
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes with no regressions
- [ ] `cargo clippy --workspace` is clean
- [ ] `docs/ARCHITECTURE.md` updated to reflect API surface and plugin system

## Notes

- Tasks 1, 2, and 4 can run in parallel (independent crates)
- Task 3 is the largest -- fdemon-app has the most visibility issues (~100+ handler functions, Engine field access, config wildcard re-export)
- The Engine field visibility change (Task 3) will require updating the headless runner and TUI runner to use accessor methods instead of direct field access
- The `EnginePlugin` trait (Task 5) should be minimal -- start with 3 methods and expand based on actual pro feature needs
- Current extension mechanism (`Engine::subscribe()`) is event-based and will remain; `EnginePlugin` adds a callback-based alternative for tighter integration
