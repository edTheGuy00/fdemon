## Task: Add Concurrent Auto-Launch Guard

**Objective**: Prevent duplicate `StartAutoLaunch` messages from spawning concurrent auto-launch tasks.

**Depends on**: Task 01

**Estimated Time**: 15 minutes

### Background

The current code doesn't check if an auto-launch is already in progress:

```rust
Message::StartAutoLaunch { configs } => {
    state.set_loading_phase("Starting...");  // Always sets loading
    UpdateResult::action(UpdateAction::DiscoverDevicesAndAutoLaunch { configs })
}
```

If a race condition triggers multiple `StartAutoLaunch` messages, concurrent discovery tasks would run, potentially causing:
- Multiple sessions being created
- Confusing UI state
- Resource contention during device discovery

### Scope

- `src/app/handler/update.rs`: Add guard check
- `src/app/handler/tests.rs`: Add test for guard

### Changes Required

**update.rs - Add guard:**
```rust
Message::StartAutoLaunch { configs } => {
    // Guard against concurrent auto-launch (already in loading mode)
    if state.ui_mode == UiMode::Loading {
        return UpdateResult::none();
    }

    // Show loading overlay on top of normal UI
    state.set_loading_phase("Starting...");
    UpdateResult::action(UpdateAction::DiscoverDevicesAndAutoLaunch { configs })
}
```

**tests.rs - Add test:**
```rust
#[test]
fn test_start_auto_launch_ignored_if_already_loading() {
    let mut state = AppState::new();
    // Simulate already in loading mode
    state.set_loading_phase("Already loading...");

    let configs = LoadedConfigs::default();
    let result = update(&mut state, Message::StartAutoLaunch { configs });

    // Should be ignored - no action spawned
    assert!(result.action.is_none());
    // Still in loading mode
    assert_eq!(state.ui_mode, UiMode::Loading);
}
```

### Acceptance Criteria

1. Guard check added at start of `StartAutoLaunch` handler
2. Second `StartAutoLaunch` while loading is silently ignored
3. Test added to verify guard behavior
4. `cargo check` passes
5. `cargo test --lib` passes (including new test)
6. `cargo clippy -- -D warnings` passes

### Testing

```bash
# Run the new test
cargo test test_start_auto_launch_ignored_if_already_loading

# Run all auto-launch tests
cargo test auto_launch
```

### Notes

- This is a defensive programming fix
- In normal usage, `StartAutoLaunch` is only sent once during startup
- The guard prevents potential issues from race conditions or edge cases
- Silent ignore (no error message) is appropriate since user won't notice
