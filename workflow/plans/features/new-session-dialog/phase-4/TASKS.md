# Phase 4: Native Device Discovery - Task Index

## Overview

Implement native device discovery for iOS simulators and Android AVDs. This phase adds the ability to list and boot offline/bootable devices using platform-specific tools.

**Total Tasks:** 10 (5 implementation + 5 review fixes)
**Estimated Time:** 3.5 hours

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-tool-availability               │
└────────────────┬────────────────────┘
                 │
        ┌────────┴────────┐
        ▼                 ▼
┌───────────────┐ ┌───────────────┐
│ 02-ios-simctl │ │ 03-android-avd│
└───────┬───────┘ └───────┬───────┘
        │                 │
        └────────┬────────┘
                 ▼
┌─────────────────────────────────────┐
│  04-boot-commands                   │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  05-discovery-integration           │
└────────────────┬────────────────────┘
                 │
    ┌────────────┼────────────┬─────────────────┐
    ▼            ▼            ▼                 ▼
┌────────┐ ┌──────────┐ ┌────────────────┐ ┌───────────────────┐
│ 06-fix │ │ 07-fix   │ │ 08-use-tool-   │ │ 09-resolve-       │
│ regex  │ │ avd-run  │ │ avail-cache    │ │ bootable-device   │
└────┬───┘ └────┬─────┘ └────────────────┘ └───────────────────┘
     │          │
     └────┬─────┘
          ▼
┌─────────────────────────────────────┐
│  10-code-quality-improvements       │
└─────────────────────────────────────┘
```

## Tasks

### Implementation Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-tool-availability](tasks/01-tool-availability.md) | Done | Phase 1 | 20m | `daemon/tool_availability.rs` |
| 2 | [02-ios-simctl](tasks/02-ios-simctl.md) | Done | 1 | 25m | `daemon/simulators.rs` |
| 3 | [03-android-avd](tasks/03-android-avd.md) | Done | 1 | 25m | `daemon/avds.rs` |
| 4 | [04-boot-commands](tasks/04-boot-commands.md) | Done | 2, 3 | 20m | `daemon/simulators.rs`, `daemon/avds.rs` |
| 5 | [05-discovery-integration](tasks/05-discovery-integration.md) | Done | 4 | 15m | `daemon/mod.rs`, `app/state.rs` |

### Review Fix Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 6 | [06-fix-regex-compilation](tasks/06-fix-regex-compilation.md) | Done | 5 | 10m | `daemon/avds.rs` |
| 7 | [07-fix-avd-running-check](tasks/07-fix-avd-running-check.md) | Done | 5 | 10m | `daemon/avds.rs` |
| 8 | [08-use-tool-availability-cache](tasks/08-use-tool-availability-cache.md) | Done | 5 | 20m | `tui/spawn.rs`, `app/message.rs`, `app/handler/update.rs` |
| 9 | [09-resolve-bootable-device-types](tasks/09-resolve-bootable-device-types.md) | Done | 5 | 25m | `daemon/mod.rs`, `core/types.rs`, `app/handler/update.rs` |
| 10 | [10-code-quality-improvements](tasks/10-code-quality-improvements.md) | Done | 6, 7 | 15m | `daemon/simulators.rs`, `daemon/avds.rs`, `daemon/tool_availability.rs`, `tui/spawn.rs` |

## Success Criteria

### Implementation Complete (Tasks 1-5)

- [x] `ToolAvailability` struct caches command availability at startup
- [x] `list_ios_simulators()` returns parsed simulator list from `xcrun simctl list -j`
- [x] `list_android_avds()` returns parsed AVD list from `emulator -list-avds`
- [x] `boot_simulator(udid)` boots iOS simulator
- [x] `boot_avd(name)` boots Android AVD
- [x] Discovery functions gracefully handle missing tools
- [x] `AppState` includes cached `ToolAvailability`

### Review Fixes Complete (Tasks 6-10)

- [x] Regex uses static initialization via `std::sync::LazyLock`
- [x] `is_avd_running()` renamed to `is_any_emulator_running()` to match actual behavior
- [x] Spawn functions use cached `ToolAvailability` from state
- [x] `BootableDevice` types unified: daemon type renamed to `BootCommand` with `From` trait conversion
- [x] Magic numbers extracted to named constants (SIMULATOR_BOOT_TIMEOUT, AVD_INIT_DELAY)
- [x] Swallowed errors have debug logging via `inspect_err()`
- [x] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

## Platform Considerations

### macOS
- `xcrun simctl` available with Xcode installation
- `emulator` available via Android SDK

### Linux
- `xcrun simctl` NOT available (iOS simulators macOS-only)
- `emulator` available via Android SDK

### Windows
- `xcrun simctl` NOT available
- `emulator.exe` available via Android SDK

## Notes

- Tool availability check runs once at app startup
- Results cached in `AppState.tool_availability`
- Discovery functions return `Result<Vec<BootableDevice>, Error>`
- Use async execution to avoid blocking UI during discovery
