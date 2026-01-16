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

**Status:** Not Started
