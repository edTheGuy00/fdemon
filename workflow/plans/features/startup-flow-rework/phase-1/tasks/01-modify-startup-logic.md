## Task: Modify Startup Logic for Normal Mode Entry

**Objective**: Change the non-auto-start startup flow to enter `UiMode::Normal` instead of showing the `StartupDialog`, allowing the app to run without requiring a device selection.

**Depends on**: None

### Scope

- `src/tui/startup.rs`: Modify `show_startup_dialog()` function

### Details

Currently, when `auto_start = false`, the `show_startup_dialog()` function is called which:
1. Sets `UiMode::StartupDialog` via `state.show_startup_dialog(configs)`
2. Spawns device discovery in background

**Changes needed:**

Replace the `show_startup_dialog()` function to enter Normal mode directly:

```rust
/// Enter normal mode without starting a session (manual mode)
///
/// User can press '+' to show the StartupDialog when ready.
fn enter_normal_mode_disconnected(state: &mut AppState) -> Option<UpdateAction> {
    // Don't show any dialog - stay in Normal mode
    // User will see "Not Connected" status and can press '+' to start
    state.ui_mode = UiMode::Normal;
    None
}
```

Then update the `startup_flutter()` function's else branch:

```rust
pub async fn startup_flutter(...) -> Option<UpdateAction> {
    // Load all configs upfront (needed for both paths)
    let configs = load_all_configs(project_path);

    if settings.behavior.auto_start {
        auto_start_session(state, &configs, project_path, msg_tx, term).await
    } else {
        // NEW: Enter normal mode directly, don't show startup dialog
        enter_normal_mode_disconnected(state)
    }
}
```

**Important**: The configs are still loaded but not used immediately. They will be loaded again when the user presses '+' to open the StartupDialog via `Message::ShowStartupDialog` handler.

### Acceptance Criteria

1. When `auto_start = false` in config, app enters `UiMode::Normal` on startup
2. No `StartupDialog` appears automatically
3. `state.session_manager.len()` is 0 (no sessions created)
4. Auto-start flow (when `auto_start = true`) continues to work unchanged
5. Device discovery is NOT spawned on startup (deferred until user action)

### Testing

Run manual verification:
```bash
# Ensure test fixture has auto_start = false
cargo run -- tests/fixtures/simple_app
# App should show Normal UI with "Waiting for Flutter..." (will be updated in task 02)
# Press 'q' to quit - should quit immediately (no confirmation since no sessions)
```

Unit test verification:
```bash
cargo test startup
cargo test --lib
```

### Notes

- The loading of configs upfront may be unnecessary now; consider lazy loading in Phase 2
- The `msg_tx` parameter is no longer needed in the non-auto-start path
- Existing tests that depend on StartupDialog appearing may need adjustment

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/startup.rs` | Replaced `show_startup_dialog()` with `enter_normal_mode_disconnected()` and updated `startup_flutter()` to call new function in non-auto-start path |

### Notable Decisions/Tradeoffs

1. **Removed Device Discovery on Non-Auto-Start**: The `spawn_device_discovery()` call is no longer made when `auto_start = false`. Device discovery is now deferred until the user explicitly requests it by pressing '+' to show the StartupDialog. This improves startup performance and reduces unnecessary background work.

2. **Configs Still Loaded Upfront**: The configs are still loaded at startup even though they're not used immediately in the non-auto-start path. This is noted in the task as potentially unnecessary and will be considered for lazy loading in Phase 2.

3. **msg_tx Parameter Now Unused in Non-Auto-Start Path**: The `msg_tx` parameter is passed to `startup_flutter()` but is no longer used when `auto_start = false`. This is acceptable as it maintains the function signature for consistency with the auto-start path.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (no compilation errors)
- `cargo clippy -- -D warnings` - Passed (no warnings)
- `cargo test --lib` - Passed (1321 tests passed, 0 failed)

### Risks/Limitations

1. **Existing Tests**: Some existing tests that depend on StartupDialog appearing automatically may need adjustment in subsequent tasks. However, all current unit tests pass, indicating no immediate breakage.
