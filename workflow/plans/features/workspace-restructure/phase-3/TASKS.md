# Phase 3: Cargo Workspace Split - Task Index

## Overview

Split the single `flutter-demon` crate into a Cargo workspace with 4 library crates (`fdemon-core`, `fdemon-daemon`, `fdemon-app`, `fdemon-tui`) and 1 binary (`fdemon`). Phase 1 cleaned all dependency violations and Phase 2 extracted the Engine abstraction, so all module boundaries now flow cleanly downward. This phase is purely structural: move files into crate directories, create `Cargo.toml` files, and update import paths.

**Total Tasks:** 10
**Estimated Hours:** 26-38 hours

## Task Dependency Graph

```
Wave 1 (parallel - no dependencies):
┌──────────────────────────────────┐  ┌──────────────────────────────────┐
│ 01-create-workspace-scaffold     │  │ 02-decouple-app-from-tui-entry   │
│ (workspace Cargo.toml, dirs)     │  │ (move run() to binary, remove    │
│                                  │  │  app/mod.rs -> tui dependency)   │
└──────────────┬───────────────────┘  └──────────────┬───────────────────┘
               │                                     │
               └──────────────┬──────────────────────┘
                              ▼
Wave 2 (sequential - each crate depends on prior):
┌──────────────────────────────────┐
│ 03-extract-fdemon-core           │
│ (common/ + core/ -> crate)       │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│ 04-extract-fdemon-daemon         │
│ (daemon/ -> crate)               │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│ 05-extract-fdemon-app            │
│ (app/ + config/ + services/      │
│  + watcher/ -> crate)            │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│ 06-extract-fdemon-tui            │
│ (tui/ -> crate)                  │
└──────────────┬───────────────────┘
               │
               ▼
Wave 3 (depends on all crate extractions):
┌──────────────────────────────────┐
│ 07-update-binary-and-headless    │
│ (main.rs, headless/ -> binary)   │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│ 08-migrate-integration-tests     │
│ (tests/ + dev-dependencies)      │
└──────────────┬───────────────────┘
               │
               ▼
Wave 4 (final verification):
┌──────────────────────────────────┐  ┌──────────────────────────────────┐
│ 09-cleanup-re-exports-and-paths  │  │ 10-verify-and-document           │
│ (remove compat bridges, audit    │  │ (cargo test, clippy, fmt, docs)  │
│  pub visibility)                 │  │                                  │
└──────────────┬───────────────────┘  └──────────────┬───────────────────┘
               │                                     │
               └──────────────┬──────────────────────┘
                              ▼
                         Phase 3 Complete
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Key Modules |
|---|------|--------|------------|------------|-------------|
| 1 | [01-create-workspace-scaffold](tasks/01-create-workspace-scaffold.md) | Done | - | 2-3h | `Cargo.toml`, `crates/*/Cargo.toml` |
| 2 | [02-decouple-app-from-tui-entry](tasks/02-decouple-app-from-tui-entry.md) | Done | - | 1-2h | `app/mod.rs`, `main.rs` |
| 3 | [03-extract-fdemon-core](tasks/03-extract-fdemon-core.md) | Done | 1, 2 | 3-5h | `common/`, `core/` -> `crates/fdemon-core/` |
| 4 | [04-extract-fdemon-daemon](tasks/04-extract-fdemon-daemon.md) | Done | 3 | 3-4h | `daemon/` -> `crates/fdemon-daemon/` |
| 5 | [05-extract-fdemon-app](tasks/05-extract-fdemon-app.md) | Done | 4 | 5-7h | `app/`, `config/`, `services/`, `watcher/` -> `crates/fdemon-app/` |
| 6 | [06-extract-fdemon-tui](tasks/06-extract-fdemon-tui.md) | Done | 5 | 3-5h | `tui/` -> `crates/fdemon-tui/` |
| 7 | [07-update-binary-and-headless](tasks/07-update-binary-and-headless.md) | Done | 6 | 2-3h | `src/main.rs`, `headless/` |
| 8 | [08-migrate-integration-tests](tasks/08-migrate-integration-tests.md) | Done | 7 | 2-3h | `tests/`, dev-dependencies |
| 9 | [09-cleanup-re-exports-and-paths](tasks/09-cleanup-re-exports-and-paths.md) | Done | 8 | 2-3h | All `pub use` re-exports, visibility audit |
| 10 | [10-verify-and-document](tasks/10-verify-and-document.md) | Done | 9 | 2-3h | `docs/ARCHITECTURE.md`, CI |

## Success Criteria

Phase 3 is complete when:

- [ ] Workspace has 4 library crates (`fdemon-core`, `fdemon-daemon`, `fdemon-app`, `fdemon-tui`) + 1 binary (`fdemon`)
- [ ] `fdemon-core` has zero internal crate dependencies
- [ ] `fdemon-daemon` depends only on `fdemon-core`
- [ ] `fdemon-app` depends only on `fdemon-core` + `fdemon-daemon`
- [ ] `fdemon-tui` depends only on `fdemon-core` + `fdemon-app` (NOT on `fdemon-daemon` directly)
- [ ] `cargo build` succeeds at workspace root
- [ ] `cargo test --workspace` passes across all crates
- [ ] `cargo clippy --workspace` is clean
- [ ] `cargo fmt --all` is consistent
- [ ] Binary produces identical `fdemon` behavior (TUI and headless modes)
- [ ] `docs/ARCHITECTURE.md` reflects the workspace structure

## Notes

- **Tasks 1 and 2 are independent** and can be done in parallel (Wave 1). Task 2 removes the only `app -> tui` call, which is a prerequisite for `fdemon-app` not depending on `fdemon-tui`.
- **Tasks 3-6 are strictly sequential** because each crate extraction depends on the prior crate existing as a dependency. Start from the leaf (`core`) and work up.
- **Task 7** wires the binary to use all 4 crates and moves `headless/` into the binary crate.
- **Task 8** migrates integration tests to work in workspace context.
- **Tasks 9-10** are cleanup and verification.
- The `config/` module merges into `fdemon-app` (not its own crate) because `LaunchConfig`/`Settings` are used pervasively by app handlers and daemon process spawning.
- The `common/` module (`error.rs`, `logging.rs`, `prelude.rs`) merges into `fdemon-core` as it's the universal foundation.
- `crossterm::event::KeyEvent` remains a dependency of `fdemon-app` for the `Message::Key` variant. Abstracting it away is deferred to Phase 4.
- `daemon/mod.rs` re-exports from `core` for backward compatibility. In the workspace, consumers import from `fdemon-core` directly; `fdemon-daemon` re-exports are removed.
- `config/settings.rs` has test-only imports of `daemon::Device`. These tests move to an integration test or the config tests construct mock device data directly.
