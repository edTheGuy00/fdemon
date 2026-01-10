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

**Status:** Not Started

**Files Modified:**
- (pending)

**Implementation Details:**

(pending)

**Testing Performed:**
- (pending)

**Notable Decisions:**
- (pending)

**Risks/Limitations:**
- (pending)
