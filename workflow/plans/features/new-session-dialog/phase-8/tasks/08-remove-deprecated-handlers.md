## Task: Remove Deprecated Message Handlers

**Objective**: Delete all deprecated message handlers from `update.rs` that only log warnings and return `UpdateResult::none()`.

**Depends on**: 07-remove-deprecated-messages

**Estimated Time**: 15 minutes

**Priority**: 🔴 Critical

**Source**: All Review Agents

### Scope

- `src/app/handler/update.rs`: Remove ~100 lines of deprecated handlers

### Problem

Deprecated handlers accept messages but produce no state changes, violating TEA purity. They create silent failure paths.

```rust
// Example of deprecated handler pattern
Message::ShowDeviceSelector => {
    warn!("ShowDeviceSelector is deprecated - use NewSessionDialog");
    UpdateResult::none()
}
```

### Handlers to Remove

Remove all match arms that contain `warn!("...deprecated...")`:

**Lines ~278-304 (DeviceSelector handlers):**
- `Message::ShowDeviceSelector`
- `Message::HideDeviceSelector`
- `Message::DeviceSelectorUp`
- `Message::DeviceSelectorDown`
- `Message::DeviceSelected(_)`
- `Message::LaunchAndroidEmulator(_)`

**Lines ~353 (RefreshDevices):**
- `Message::RefreshDevices`

**Lines ~374-381 (Emulator UI):**
- Emulator-related deprecated handlers

**Lines ~696-792 (StartupDialog handlers):**
- `Message::ShowStartupDialog`
- `Message::HideStartupDialog`
- `Message::StartupDialogUp`
- `Message::StartupDialogDown`
- `Message::StartupDialogNextSection`
- `Message::StartupDialogPrevSection`
- `Message::StartupDialogNextSectionSkipDisabled`
- `Message::StartupDialogPrevSectionSkipDisabled`
- `Message::StartupDialogSelectConfig(_)`
- `Message::StartupDialogSelectDevice(_)`
- `Message::StartupDialogSetMode(_)`
- `Message::StartupDialogCharInput(_)`
- `Message::StartupDialogBackspace`
- `Message::StartupDialogClearInput`
- `Message::StartupDialogConfirm`
- `Message::SaveStartupDialogConfig`
- `Message::StartupDialogRefreshDevices`
- `Message::StartupDialogJumpToSection(_)`
- `Message::StartupDialogEnterEdit`
- `Message::StartupDialogExitEdit`

### Process

1. After Task 07 completes, these handlers will have dead match arms
2. `cargo check` will show "unreachable pattern" warnings or errors
3. Delete all match arms for removed message variants
4. Remove any associated helper functions that become unused

### Acceptance Criteria

1. `cargo check` passes
2. No warning-only handlers remain in `update.rs`
3. `update.rs` reduced by ~100 lines
4. No dead code warnings for handler functions
5. `cargo clippy` passes

### Testing

Compilation verifies correctness - handlers for non-existent messages cannot exist.

### Notes

- This task is straightforward after Task 07 - just delete the match arms
- May also need to remove deprecated helper functions
- The file should become significantly smaller and cleaner

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/update.rs` | No changes required - all deprecated warning-only handlers were already removed in Task 07 |

### Notable Decisions/Tradeoffs

1. **Task Already Complete**: Task 07 already removed all deprecated message handlers that only logged warnings and returned `UpdateResult::none()`. The message variants were deleted, making the match arms unreachable and causing compilation errors, which forced their removal.

2. **Functional Handlers Preserved**: Two handlers that contain deprecation warnings remain (`EmulatorsDiscovered` and `EmulatorDiscoveryFailed`), but these are NOT "warning-only" handlers. They perform functional work by logging discovery results and errors, then add a deprecation note about the Emulator UI being deprecated. These handlers must remain because their message variants still exist in `message.rs` and are used by the discovery infrastructure.

3. **No Dead Code**: All deprecated message handlers that were pure stubs (ShowDeviceSelector, HideDeviceSelector, DeviceSelectorUp, DeviceSelectorDown, DeviceSelected, LaunchAndroidEmulator, RefreshDevices, and all StartupDialog handlers) were already removed in Task 07.

### Testing Performed

- `cargo check` - Passed (0.39s)
- `cargo clippy -- -D warnings` - Passed (0.14s)
- `cargo build` - No unused code warnings
- Manual verification - No deprecated warning-only handlers remain

### Risks/Limitations

1. **No Risks**: The task was already completed by Task 07. The codebase is in the correct state with no deprecated stub handlers remaining.

2. **Emulator Handlers Are Functional**: The remaining handlers with deprecation notes are functional and required. They cannot be removed without also removing the message variants from `message.rs`, which would break the emulator discovery infrastructure.
