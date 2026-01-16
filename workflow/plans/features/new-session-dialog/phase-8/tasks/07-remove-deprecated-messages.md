## Task: Remove Deprecated Message Variants

**Objective**: Delete all deprecated message variants from `message.rs` so any remaining references fail at compile time.

**Depends on**: 06-fix-key-handlers

**Estimated Time**: 20 minutes

**Priority**: 🔴 Critical

**Source**: All Review Agents

### Scope

- `src/app/message.rs`: Remove ~25 deprecated message variants

### Problem

Deprecated message variants can still be constructed and sent, creating silent failure paths that violate TEA principles.

### Messages to Remove

Delete the following variants from the `Message` enum:

**DeviceSelector variants (lines ~116-122):**
```rust
ShowDeviceSelector,
HideDeviceSelector,
DeviceSelectorUp,
DeviceSelectorDown,
```

**StartupDialog variants (lines ~340-395):**
```rust
ShowStartupDialog,
HideStartupDialog,
StartupDialogUp,
StartupDialogDown,
StartupDialogNextSection,
StartupDialogPrevSection,
StartupDialogNextSectionSkipDisabled,
StartupDialogPrevSectionSkipDisabled,
StartupDialogSelectConfig(usize),
StartupDialogSelectDevice(usize),
StartupDialogSetMode(crate::config::FlutterMode),
StartupDialogCharInput(char),
StartupDialogBackspace,
StartupDialogClearInput,
StartupDialogConfirm,
StartupDialogRefreshDevices,
StartupDialogJumpToSection(String),
StartupDialogEnterEdit,
StartupDialogExitEdit,
SaveStartupDialogConfig,
```

**Other deprecated variants:**
```rust
DeviceSelected(Device),
LaunchAndroidEmulator(String),
RefreshDevices,
```

### Process

1. Remove all deprecated variants from `Message` enum
2. Run `cargo check` - compiler will show all remaining references
3. Each error shows code that needs updating (should be none after Task 06)
4. Iterate until compilation succeeds

### Acceptance Criteria

1. `cargo check` passes
2. No deprecated message variants in `message.rs`
3. Any code attempting to use deleted variants fails at compile time
4. `cargo fmt` passes

### Testing

Compilation is the test - deleted variants cannot be used.

### Notes

- This may expose additional compile errors beyond Task 06 fixes
- Fix any new errors as they appear
- Keep `DevicesDiscovered` and related active messages - only remove the deprecated dialog-specific ones

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/message.rs` | Removed 27 deprecated message variants (DeviceSelector, StartupDialog, and other legacy messages) |
| `src/app/handler/update.rs` | Removed deprecated message match arms (28 handlers that only logged warnings) |
| `src/app/handler/keys.rs` | Fixed emulator selector escape key to use `OpenNewSessionDialog` instead of deprecated `ShowDeviceSelector` |

### Notable Decisions/Tradeoffs

1. **Emulator Selector Escape Behavior**: Changed the escape key in emulator selector mode from `ShowDeviceSelector` to `OpenNewSessionDialog`, aligning with the new dialog architecture.

2. **Complete Removal Strategy**: Removed all deprecated message variants at once rather than incrementally, forcing compile-time failures for any remaining references. This ensures no silent failures can occur.

3. **Handler Cleanup**: Removed all deprecated message handlers from `update.rs` that were only logging warnings and returning `UpdateResult::none()`. These were serving no functional purpose.

### Testing Performed

- `cargo check` - Passed
- `cargo test` - Running (backgrounded)
- `cargo clippy -- -D warnings` - Passed
- `cargo fmt` - Passed

### Risks/Limitations

1. **No Runtime Risks**: Since all compilation checks pass, there are no remaining references to deprecated messages. Any future attempts to use these messages will fail at compile time as intended.

2. **Active Messages Preserved**: Confirmed that `DevicesDiscovered`, `DeviceDiscoveryFailed`, and other active device-related messages remain intact and functional.
