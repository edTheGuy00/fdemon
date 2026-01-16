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

**Status:** Not Started
