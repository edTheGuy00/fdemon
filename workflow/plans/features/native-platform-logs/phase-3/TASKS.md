# Phase 3: Custom Log Sources + Documentation & Website — Task Index

## Overview

Add user-configurable custom log source processes, update example project documentation, and create comprehensive user-facing documentation including a dedicated website page for native platform logs.

**Total Tasks:** 9

## Task Dependency Graph

```
Stream A — Custom Log Sources          Stream B — Examples       Stream C — Documentation
──────────────────────────────         ──────────────────        ─────────────────────────

Wave 1 (parallel — no dependencies):
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│ 01-custom-source │  │ 02-format-       │  │ 06-example-      │
│   -config        │  │   parsers        │  │   project-updates│
│ Config types +   │  │ Raw/Json/Logcat/ │  │ READMEs + sample │
│ TOML parsing     │  │ Syslog parsers   │  │ .fdemon/config   │
└────────┬─────────┘  └────────┬─────────┘  └──────────────────┘
         │                     │
Wave 2 (depends on 01 + 02):  │
         └──────────┬──────────┘
                    ▼
         ┌──────────────────┐
         │ 03-custom-source │
         │   -runner        │
         │ CustomLogCapture │
         │ implementation   │
         └────────┬─────────┘
                  │
Wave 3 (depends on 03):
                  ▼
         ┌──────────────────┐
         │ 04-app-custom-   │
         │   source-        │
         │   integration    │
         │ Session lifecycle│
         │ + tag filter     │
         └────────┬─────────┘
                  │
Wave 4 (depends on 03):
                  ▼
         ┌──────────────────┐
         │ 05-custom-source │
         │   -tests         │
         │ Comprehensive    │
         │ test coverage    │
         └────────┬─────────┘
                  │
                  └──────────────────────────────────────────┐
                                                             │
Wave 5 (depends on 04+05; parallel with each other):        │
┌──────────────────┐  ┌──────────────────┐  ┌───────────────┴┐  ┌──────────────────┐
│ 07-docs-         │  │ 08-website-      │  │ 09-docs-       │  │ (06 independent) │
│   configuration  │  │   native-logs-   │  │   architecture │  │                  │
│ CONFIGURATION.md │  │   page           │  │ ARCHITECTURE.md│  │                  │
│ [native_logs]    │  │ New Leptos page  │  │ custom sources │  │                  │
│ reference        │  │ + route + sidebar│  │ subsystem      │  │                  │
└──────────────────┘  └──────────────────┘  └────────────────┘  └──────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-custom-source-config](tasks/01-custom-source-config.md) | Done | - | `fdemon-app/src/config/types.rs` |
| 2 | [02-format-parsers](tasks/02-format-parsers.md) | Done | - | `fdemon-daemon/src/native_logs/formats.rs` (NEW) |
| 3 | [03-custom-source-runner](tasks/03-custom-source-runner.md) | Done | 1, 2 | `fdemon-daemon/src/native_logs/custom.rs` (NEW) |
| 4 | [04-app-custom-source-integration](tasks/04-app-custom-source-integration.md) | Done | 3 | `fdemon-app/src/actions/native_logs.rs`, `session/handle.rs`, `handler/session.rs` |
| 5 | [05-custom-source-tests](tasks/05-custom-source-tests.md) | Done | 3 | All modified files |
| 6 | [06-example-project-updates](tasks/06-example-project-updates.md) | Done | - | `example/app1/`, `example/app2/` |
| 7 | [07-docs-configuration](tasks/07-docs-configuration.md) | Done | 4 | `docs/CONFIGURATION.md` |
| 8 | [08-website-native-logs-page](tasks/08-website-native-logs-page.md) | Done | 4 | `website/src/pages/docs/native_logs.rs` (NEW), `mod.rs`, `lib.rs`, `data.rs` |
| 9 | [09-docs-architecture](tasks/09-docs-architecture.md) | Done | 4 | `docs/ARCHITECTURE.md` |

## Execution Plan

- **Wave 1** (tasks 01, 02, 06): All independent — dispatch in parallel
- **Wave 2** (task 03): Depends on 01 + 02 — dispatch when both complete
- **Wave 3** (task 04): Depends on 03 — dispatch when complete
- **Wave 4** (task 05): Depends on 03 — can run in parallel with 04 if runner is stable
- **Wave 5** (tasks 07, 08, 09): Depend on implementation being complete — dispatch in parallel

**Critical path (custom sources):** 01 + 02 → 03 → 04
**Independent (examples):** 06 can run any time
**Documentation (parallel):** 07 + 08 + 09 after implementation complete

## Success Criteria

Phase 3 is complete when:

- [ ] `[[native_logs.custom_sources]]` TOML config parsed and validated
- [ ] All 4 format parsers work: raw, json, logcat-threadtime, syslog
- [ ] Custom source processes spawned alongside platform capture after `AppStarted`
- [ ] Custom source tags appear in tag filter UI (`T` key overlay)
- [ ] Custom source processes shut down cleanly on session end
- [ ] Process exit/crash logged as warning (no silent failures)
- [ ] `example/app1/README.md` and `example/app2/README.md` describe what each app demonstrates
- [ ] `example/app2/` includes sample `.fdemon/config.toml` with custom source config
- [ ] `docs/CONFIGURATION.md` has full `[native_logs]` reference including custom sources
- [ ] Website has `/docs/native-logs` page with full feature docs
- [ ] `T` key in website keybindings data
- [ ] `docs/ARCHITECTURE.md` updated with custom source subsystem
- [ ] All new code has unit tests
- [ ] No regressions in existing native log pipeline (Android, macOS, iOS)
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

## Notes

- Custom sources reuse the existing `NativeLogCapture` trait and `NativeLogEvent` type — no new core types needed
- Format parsers for `logcat-threadtime` and `syslog` should reuse the existing parsers from `android.rs` and `macos.rs` — extract or delegate, don't duplicate
- Custom source tags integrate with the existing `NativeTagState` and tag filter overlay — no UI changes needed beyond what Phase 2 delivered
- Custom sources do NOT auto-restart on crash — this keeps the implementation simple and avoids runaway process spawning from bad user config
- Commands are spawned via `tokio::process::Command::new()` with explicit args — never via `sh -c` (no shell expansion)
- The website is a Leptos (Rust/WASM) SPA — the new docs page follows the same component pattern as existing pages (`Section`, `SettingsTable`, `CodeBlock`, `Tip`)
- `docs/CONFIGURATION.md` currently has no `[native_logs]` section — this needs to be added as a complete reference
- The `T` key is documented in `docs/KEYBINDINGS.md` but missing from `website/src/data.rs` — needs to be added
