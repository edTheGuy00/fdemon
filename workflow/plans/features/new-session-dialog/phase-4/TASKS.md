# Phase 4: Native Device Discovery - Task Index

## Overview

Implement native device discovery for iOS simulators and Android AVDs. This phase adds the ability to list and boot offline/bootable devices using platform-specific tools.

**Total Tasks:** 5
**Estimated Time:** 2 hours

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
└─────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-tool-availability](tasks/01-tool-availability.md) | Not Started | Phase 1 | 20m | `daemon/tool_availability.rs` |
| 2 | [02-ios-simctl](tasks/02-ios-simctl.md) | Not Started | 1 | 25m | `daemon/simulators.rs` |
| 3 | [03-android-avd](tasks/03-android-avd.md) | Not Started | 1 | 25m | `daemon/avds.rs` |
| 4 | [04-boot-commands](tasks/04-boot-commands.md) | Not Started | 2, 3 | 20m | `daemon/simulators.rs`, `daemon/avds.rs` |
| 5 | [05-discovery-integration](tasks/05-discovery-integration.md) | Not Started | 4 | 15m | `daemon/mod.rs`, `app/state.rs` |

## Success Criteria

Phase 4 is complete when:

- [ ] `ToolAvailability` struct caches command availability at startup
- [ ] `list_ios_simulators()` returns parsed simulator list from `xcrun simctl list -j`
- [ ] `list_android_avds()` returns parsed AVD list from `emulator -list-avds`
- [ ] `boot_simulator(udid)` boots iOS simulator
- [ ] `boot_avd(name)` boots Android AVD
- [ ] Discovery functions gracefully handle missing tools
- [ ] `AppState` includes cached `ToolAvailability`
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

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
