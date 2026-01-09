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

**Status:** Not Started

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**
(To be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo check` - Pending
- `cargo clippy` - Pending
- `cargo test` - Pending
