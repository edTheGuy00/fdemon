# Phase 1: Multi-Strategy SDK Locator - Task Index

## Overview

Replace `Command::new("flutter")` with a robust, multi-strategy SDK discovery system that works with all major version managers (FVM, Puro, asdf, mise, proto, flutter_wrapper) out of the box.

**Total Tasks:** 7

## Task Dependency Graph

```
┌─────────────────────────┐     ┌─────────────────────────┐
│  01-core-types          │     │  05-flutter-settings    │
│  (types, mod, deps)     │     │  (config.toml section)  │
└─────────┬───────────────┘     └───────────┬─────────────┘
          │                                 │
   ┌──────┼──────────────┐                  │
   │      │              │                  │
   ▼      ▼              ▼                  │
┌──────┐ ┌──────┐ ┌──────────────┐          │
│  02  │ │  03  │ │     06       │          │
│ ver. │ │ chan │ │  call sites  │          │
│ mgrs │ │ info │ │  (daemon)   │          │
└──┬───┘ └──┬───┘ └──────┬──────┘          │
   │        │             │                 │
   └────┬───┘             │                 │
        ▼                 │                 │
   ┌──────────┐           │                 │
   │    04    │           │                 │
   │  locator │           │                 │
   └────┬─────┘           │                 │
        │                 │                 │
        └────────┬────────┘─────────────────┘
                 ▼
        ┌────────────────┐
        │       07       │
        │  engine/state  │
        │  integration   │
        └────────────────┘
```

### Parallelism Waves

| Wave | Tasks | Can Run In Parallel |
|------|-------|-------------------|
| 1 | 01, 05 | Yes |
| 2 | 02, 03, 06 | Yes (all depend on 01 only) |
| 3 | 04 | No (depends on 02, 03) |
| 4 | 07 | No (depends on 04, 05, 06) |

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-core-types](tasks/01-core-types.md) | Done | - | `fdemon-daemon: flutter_sdk/types.rs, mod.rs, Cargo.toml, lib.rs` `fdemon-core: error.rs` |
| 2 | [02-version-manager-parsers](tasks/02-version-manager-parsers.md) | Done | 01 | `fdemon-daemon: flutter_sdk/version_managers.rs` |
| 3 | [03-channel-version-extraction](tasks/03-channel-version-extraction.md) | Done | 01 | `fdemon-daemon: flutter_sdk/channel.rs` |
| 4 | [04-sdk-locator](tasks/04-sdk-locator.md) | Done | 02, 03 | `fdemon-daemon: flutter_sdk/locator.rs` |
| 5 | [05-flutter-settings](tasks/05-flutter-settings.md) | Done | - | `fdemon-app: config/types.rs, config/settings.rs` |
| 6 | [06-update-call-sites](tasks/06-update-call-sites.md) | Done | 01 | `fdemon-daemon: process.rs, devices.rs, emulators.rs, lib.rs` |
| 7 | [07-engine-state-integration](tasks/07-engine-state-integration.md) | Done | 04, 05, 06 | `fdemon-app: state.rs, engine.rs, message.rs, handler/update.rs, handler/mod.rs` `fdemon-daemon: tool_availability.rs` |

## Success Criteria

Phase 1 is complete when:

- [ ] fdemon detects Flutter installed via FVM (v2 and v3), Puro, asdf, mise, proto, flutter_wrapper, and manual installation
- [ ] `FLUTTER_ROOT` env var is respected as highest-priority auto-detection
- [ ] `config.toml` `flutter.sdk_path` overrides all detection
- [ ] All three call sites (`process.rs`, `devices.rs`, `emulators.rs`) use the resolved `FlutterSdk`
- [ ] Windows `.bat` wrapper files are handled correctly via `FlutterExecutable::WindowsBatch`
- [ ] Detection chain logged at `debug` level for troubleshooting
- [ ] Directory tree walk finds config files in parent directories (monorepo support)
- [ ] `ToolAvailability` includes Flutter SDK check at startup
- [ ] Comprehensive unit tests for each detection strategy
- [ ] Existing tests pass, no regressions
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

## Notes

- **`Engine::new()` is synchronous.** SDK resolution uses only synchronous filesystem operations (file existence checks, reads, symlink resolution). No async `Command` calls are needed — the locator does not invoke `flutter --version` or any external process. It reads config files and validates directory structure.
- **`fdemon-daemon` needs two new workspace deps:** `toml.workspace = true` (for `.mise.toml` parsing) and `dirs.workspace = true` (for home directory resolution like `~/fvm/versions/`).
- **`serde_json` is already in fdemon-daemon** — covers `.fvmrc` and `.puro.json` parsing.
- **No `flutter_locator.rs` from PR #19 exists** in the current codebase. We start fresh.
- **The `FlutterNotFound` error variant already exists** in `fdemon-core/error.rs` — we extend it rather than replace it.
