# Phase 1: Android + macOS Native Logs — Task Index

## Overview

Capture and display native platform log output alongside Flutter logs on Android (via `adb logcat`) and macOS (via `log stream`), with source-based filtering and configuration. Linux/Windows/Web are unaffected — their existing stdout/stderr pipes already cover native output.

**Total Tasks:** 9

## Task Dependency Graph

```
Wave 1 (parallel — no dependencies):
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  01-core-types   │  │ 02-native-log-   │  │ 03-tool-         │  │ 09-example-app-  │
│  LogSource::     │  │    config        │  │    availability  │  │   native-logs    │
│  Native, filter  │  │  NativeLogsSett  │  │  adb + log cmd   │  │ Kotlin + Swift   │
└────────┬─────────┘  └────────┬─────────┘  └────────┬─────────┘  └──────────────────┘
         │                     │                     │
Wave 2:  │                     │                     │
         ▼                     │                     │
┌──────────────────┐           │                     │
│ 04-shared-native │           │                     │
│    -infra        │           │                     │
│ NativeLogEvent,  │           │                     │
│ trait, dispatch   │           │                     │
└───────┬──────────┘           │                     │
        │                      │                     │
        │  ┌───────────────────┘                     │
        │  │  ┌──────────────────────────────────────┘
        │  │  │
Wave 2 (parallel with 04):    │
        │  │  │  ┌──────────────────┐
        │  │  │  │ 08-tui-rendering │  (depends on 01 only)
        │  │  │  │ palette + style  │
        │  │  │  └──────────────────┘
        │  │  │
Wave 3 (parallel — depend on 04):
        ▼  │  │
┌──────────────────┐  ┌──────────────────┐
│ 05-android-      │  │ 06-macos-log-    │
│    logcat        │  │    stream        │
│ adb logcat spawn │  │ log stream spawn │
│ threadtime parse │  │ syslog parse     │
└───────┬──────────┘  └────────┬─────────┘
        │                      │
        └──────────┬───────────┘
                   │
Wave 4:            ▼
         ┌──────────────────┐
         │ 07-app-          │
         │    integration   │  (depends on 01, 02, 03, 04, 05, 06)
         │ Message variant, │
         │ action, routing  │
         └──────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-core-types](tasks/01-core-types.md) | Done | - | `fdemon-core/src/types.rs` |
| 2 | [02-native-log-config](tasks/02-native-log-config.md) | Done | - | `fdemon-app/src/config/types.rs`, `settings.rs` |
| 3 | [03-tool-availability](tasks/03-tool-availability.md) | Done | - | `fdemon-daemon/src/tool_availability.rs` |
| 4 | [04-shared-native-infra](tasks/04-shared-native-infra.md) | Done | 1 | `fdemon-daemon/src/native_logs/mod.rs` |
| 5 | [05-android-logcat](tasks/05-android-logcat.md) | Done | 4 | `fdemon-daemon/src/native_logs/android.rs` |
| 6 | [06-macos-log-stream](tasks/06-macos-log-stream.md) | Done | 4 | `fdemon-daemon/src/native_logs/macos.rs` |
| 7 | [07-app-integration](tasks/07-app-integration.md) | Done | 1, 2, 3, 4, 5, 6 | `fdemon-app/src/` (message, handler, actions, session) |
| 8 | [08-tui-rendering](tasks/08-tui-rendering.md) | Done | 1 | `fdemon-tui/src/theme/palette.rs`, `widgets/log_view/mod.rs` |
| 9 | [09-example-app-native-logs](tasks/09-example-app-native-logs.md) | Done | - | `example/app2/` (Kotlin, Swift, Dart) |

## Execution Plan

- **Wave 1** (tasks 01, 02, 03, 09): All independent — dispatch in parallel
- **Wave 2** (tasks 04, 08): Task 04 depends on 01; task 08 depends on 01 — dispatch when 01 completes
- **Wave 3** (tasks 05, 06): Both depend on 04 — dispatch in parallel when 04 completes
- **Wave 4** (task 07): Depends on all prior tasks — dispatch last

**Critical path:** 01 → 04 → 05/06 → 07

## Success Criteria

Phase 1 is complete when:

- [ ] Android native logcat output appears in fdemon log view with `[tag]` prefix
- [ ] macOS native `NSLog`/`os_log` output appears in fdemon log view
- [ ] `LogSourceFilter` cycles through `Native` and can toggle native logs on/off
- [ ] `flutter` tag excluded by default to avoid duplication
- [ ] PID-based filtering works on Android; graceful fallback if PID unavailable
- [ ] Process-name filtering works on macOS
- [ ] Native log capture starts after `AppStarted` and stops on session end
- [ ] Tool availability checked; graceful degradation if `adb`/`log` missing
- [ ] Linux/Windows/Web sessions are unaffected (no native capture attempted)
- [ ] Configurable via `[native_logs]` section in config.toml
- [ ] All new code has unit tests
- [ ] No regressions in existing log pipeline
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes
- [ ] `example/app2` has native Android + macOS logging for manual testing

## Notes

- `DaemonEvent` lives in `fdemon-core/src/events.rs` (not daemon crate) — new native log events may go there or use a separate channel pattern
- The existing `watch::channel<bool>` + `JoinHandle` shutdown pattern (used by perf/network polling) should be reused for native log tasks
- `LogSource::VmService` is grouped under `Flutter` filter — `LogSource::Native` gets its own dedicated filter variant
- Session stores device platform as `String` (e.g., `"android"`, `"macos"`) — use this for platform dispatch
- macOS code must be gated with `#[cfg(target_os = "macos")]`
