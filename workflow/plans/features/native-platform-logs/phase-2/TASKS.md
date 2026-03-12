# Phase 2: iOS Native Logs + Per-Tag Filtering — Task Index

## Overview

Add iOS native log capture (physical devices via `idevicesyslog`, simulators via `xcrun simctl spawn log stream`) and a per-tag filtering UI that lets users toggle individual native log tags on/off. Also adds per-tag priority thresholds in configuration.

**Total Tasks:** 10

## Task Dependency Graph

```
Stream A — iOS Native Logs          Stream B — Per-Tag Filtering
─────────────────────────            ──────────────────────────

Wave 1 (parallel — no dependencies):
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│ 01-ios-tool-     │  │ 02-ios-log-      │  │ 06-example-app-  │  │ 07-per-tag-      │  │ 08-per-tag-      │
│   availability   │  │   config         │  │   ios            │  │   state          │  │   config         │
│ idevicesyslog +  │  │ IosLogConfig +   │  │ Swift native log │  │ Discovered tags  │  │ Per-tag level    │
│ simctl log check │  │ "ios" dispatch   │  │ in example/app2  │  │ + filter state   │  │ thresholds       │
└────────┬─────────┘  └────────┬─────────┘  └──────────────────┘  └────────┬─────────┘  └────────┬─────────┘
         │                     │                                           │                     │
Wave 2 (depend on 02):         │                                           │                     │
         │  ┌──────────────────┤                                           │                     │
         │  │                  │                                           │                     │
         │  ▼                  ▼                                           │                     │
         │  ┌──────────────────┐  ┌──────────────────┐                     │                     │
         │  │ 03-ios-simulator │  │ 04-ios-physical  │                     │                     │
         │  │   capture        │  │   capture        │                     │                     │
         │  │ simctl log stream│  │ idevicesyslog    │                     │                     │
         │  └───────┬──────────┘  └────────┬─────────┘                     │                     │
         │          │                      │                               │                     │
         │          └──────────┬───────────┘                               │                     │
         │                     │                                           │                     │
Wave 3:  │                     ▼                                           ▼                     │
         │          ┌──────────────────┐                        ┌──────────────────┐             │
         └─────────▶│ 05-app-ios-      │                        │ 09-per-tag-      │◀────────────┘
                    │   integration    │                        │   filter-ui      │
                    │ Session handler  │                        │ Tag popup + `T`  │
                    │ + actions wiring │                        │ key binding      │
                    └───────┬──────────┘                        └────────┬─────────┘
                            │                                           │
                            └──────────────────┬────────────────────────┘
                                               │
Wave 4:                                        ▼
                                    ┌──────────────────┐
                                    │ 10-docs-update   │
                                    │ ARCHITECTURE.md  │
                                    │ KEYBINDINGS.md   │
                                    └──────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-ios-tool-availability](tasks/01-ios-tool-availability.md) | Done | - | `fdemon-daemon/src/tool_availability.rs` |
| 2 | [02-ios-log-config](tasks/02-ios-log-config.md) | Done | - | `fdemon-daemon/src/native_logs/mod.rs`, `lib.rs` |
| 3 | [03-ios-simulator-capture](tasks/03-ios-simulator-capture.md) | Done | 2 | `fdemon-daemon/src/native_logs/ios.rs` |
| 4 | [04-ios-physical-capture](tasks/04-ios-physical-capture.md) | Done | 2 | `fdemon-daemon/src/native_logs/ios.rs` |
| 5 | [05-app-ios-integration](tasks/05-app-ios-integration.md) | Done | 1, 2, 3, 4 | `fdemon-app/src/handler/session.rs`, `actions/native_logs.rs` |
| 6 | [06-example-app-ios](tasks/06-example-app-ios.md) | Done | - | `example/app2/ios/` |
| 7 | [07-per-tag-state](tasks/07-per-tag-state.md) | Done | - | `fdemon-app/src/session/`, `handler/update.rs` |
| 8 | [08-per-tag-config](tasks/08-per-tag-config.md) | Done | - | `fdemon-app/src/config/types.rs` |
| 9 | [09-per-tag-filter-ui](tasks/09-per-tag-filter-ui.md) | Done | 7, 8 | `fdemon-tui/src/widgets/`, `fdemon-app/src/handler/` |
| 10 | [10-docs-update](tasks/10-docs-update.md) | Done | 5, 9 | `docs/ARCHITECTURE.md`, `docs/KEYBINDINGS.md` |

## Execution Plan

- **Wave 1** (tasks 01, 02, 06, 07, 08): All independent — dispatch in parallel
- **Wave 2** (tasks 03, 04): Both depend on 02 — dispatch in parallel when 02 completes
- **Wave 3** (tasks 05, 09): Task 05 depends on 01+02+03+04; task 09 depends on 07+08 — dispatch in parallel when deps complete
- **Wave 4** (task 10): Depends on 05 and 09 — dispatch last

**Critical path (iOS):** 02 → 03/04 → 05
**Critical path (tags):** 07+08 → 09

## Success Criteria

Phase 2 is complete when:

- [ ] iOS simulator native logs captured via `xcrun simctl spawn <udid> log stream` and displayed in fdemon
- [ ] iOS physical device native logs captured via `idevicesyslog -u <udid> -p Runner` and displayed in fdemon
- [ ] Tool availability checked for iOS; graceful degradation if `idevicesyslog`/`simctl` missing
- [ ] Per-tag filter UI shows discovered tags with toggle on/off
- [ ] `T` keybinding opens tag filter overlay in log view
- [ ] Per-tag priority thresholds configurable in `[native_logs.tags.<tag>]`
- [ ] `example/app2` has iOS native logging for manual testing
- [ ] All new code has unit tests
- [ ] No regressions in existing native log pipeline (Android + macOS)
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

## Notes

- iOS simulator `log stream` uses the same syslog format as macOS — the `parse_syslog_line()` function from `macos.rs` can be extracted and shared
- Physical device `idevicesyslog` uses BSD syslog format which is different and needs its own parser
- `idevicesyslog` is broken on Xcode 26; for now, target Xcode 15/16 support. Xcode 26 physical device log capture is deferred (Flutter itself is still figuring this out via PR #173724)
- All iOS-specific code should be gated with `#[cfg(target_os = "macos")]` since fdemon can only target iOS devices from a macOS host
- The `IosLogCapture` struct uses an `is_simulator` flag to choose between `simctl log stream` and `idevicesyslog` at runtime
- Session stores device platform as `"ios"` for iOS targets — used for platform dispatch
- Per-tag filtering state is per-session, stored alongside the existing `native_log_shutdown_tx` on `SessionHandle`
