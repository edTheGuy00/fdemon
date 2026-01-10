## Task: Add DiscoverDevicesAndAutoLaunch Action

**Objective**: Add a new `UpdateAction` variant that triggers the async auto-launch task.

**Depends on**: 01-add-message-variants

**Estimated Time**: 0.5 hours

### Scope

- `src/app/handler/mod.rs`: Add new action variant to `UpdateAction` enum

### Details

Add the following variant to the `UpdateAction` enum:

```rust
/// Discover devices and auto-launch a session
/// Used when auto_start=true to run device discovery in background
/// and automatically launch with the best available config/device
DiscoverDevicesAndAutoLaunch {
    /// Pre-loaded configs for selection logic
    configs: crate::config::LoadedConfigs,
},
```

### Location

Find the `UpdateAction` enum in `src/app/handler/mod.rs`. The enum currently has variants like:
- `SpawnSession`
- `SpawnTask`
- `DiscoverDevices`
- `DiscoverEmulators`
- `LaunchEmulator`

Add the new variant in a logical location (near `DiscoverDevices`).

### Acceptance Criteria

1. `UpdateAction::DiscoverDevicesAndAutoLaunch` variant exists
2. Variant has `configs: LoadedConfigs` field
3. Variant is documented with `///` comments
4. `cargo check` passes
5. `cargo clippy -- -D warnings` passes

### Testing

No unit tests needed for this task (enum doesn't have logic).
Compilation verification is sufficient.

```bash
cargo check
cargo clippy -- -D warnings
```

### Notes

- This action will be handled in `src/tui/actions.rs` (Phase 1, Task 4)
- The pattern follows existing actions like `DiscoverDevices`
- May need to add import for `LoadedConfigs` if not already present

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/mod.rs` | Added `UpdateAction::DiscoverDevicesAndAutoLaunch` variant with `configs: LoadedConfigs` field and documentation comments. Updated imports to include `LoadedConfigs` from `crate::config`. |
| `src/tui/actions.rs` | Added stub match arm for `DiscoverDevicesAndAutoLaunch` variant to satisfy Rust's exhaustive matching requirement. Includes TODO comment referencing Phase 1, Task 4 for full implementation. |

### Notable Decisions/Tradeoffs

1. **Import Strategy**: Modified the import in `src/app/handler/mod.rs` from `use crate::config::LaunchConfig;` to `use crate::config::{LaunchConfig, LoadedConfigs};` to minimize import lines while maintaining clarity.
2. **Stub Implementation**: Added a minimal stub handler in `src/tui/actions.rs` that temporarily calls `spawn::spawn_device_discovery(msg_tx)` to satisfy compilation requirements. This will be replaced in Phase 1, Task 4 with the full auto-launch logic.
3. **Placement**: Placed the new variant directly after `DiscoverDevices` as specified in the task, maintaining logical grouping of device-discovery-related actions.

### Testing Performed

- `cargo check` - Passed
- `cargo clippy -- -D warnings` - Passed (no warnings)
- `cargo test --lib` - Passed (1330 passed; 0 failed; 3 ignored)
- `cargo fmt` - Applied (code formatted according to project standards)

### Risks/Limitations

1. **Stub Implementation**: The current handler in `src/tui/actions.rs` only discovers devices and does not implement auto-launch logic. This is intentional and will be addressed in Phase 1, Task 4.
2. **Unused Field Warning**: The `configs` field is currently unused in the stub implementation (silenced with `configs: _` pattern). This is expected and will be utilized in Task 4.
