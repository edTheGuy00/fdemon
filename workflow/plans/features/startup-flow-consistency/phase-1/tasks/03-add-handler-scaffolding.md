## Task: Add Handler Scaffolding for Auto-Launch Messages

**Objective**: Add handler match arms for the new auto-launch messages. Initially these can be minimal/logging-only; full logic will be added in Phase 3.

**Depends on**: 02-add-update-action

**Estimated Time**: 1-2 hours

### Scope

- `src/app/handler/update.rs`: Add match arms for new messages

### Details

Add handlers for the three new message variants in the main `update()` function:

#### 1. StartAutoLaunch Handler

```rust
Message::StartAutoLaunch { configs } => {
    // Phase 1: Scaffolding only - set loading state and return action
    // Full logic will be refined in Phase 3

    // Enter loading mode
    state.set_loading_phase("Starting...");

    // Return action to spawn the auto-launch task
    UpdateResult::action(UpdateAction::DiscoverDevicesAndAutoLaunch { configs })
}
```

#### 2. AutoLaunchProgress Handler

```rust
Message::AutoLaunchProgress { message } => {
    // Update loading screen message
    state.update_loading_message(&message);
    UpdateResult::none()
}
```

#### 3. AutoLaunchResult Handler

```rust
Message::AutoLaunchResult { result } => {
    match result {
        Ok(success) => {
            // Create session and spawn
            let AutoLaunchSuccess { device, config } = success;

            let session_result = if let Some(cfg) = &config {
                state.session_manager.create_session_with_config(&device, cfg.clone())
            } else {
                state.session_manager.create_session(&device)
            };

            match session_result {
                Ok(session_id) => {
                    // Clear loading, enter normal mode
                    state.clear_loading();
                    state.ui_mode = UiMode::Normal;

                    // Save selection for next time
                    let _ = crate::config::save_last_selection(
                        &state.project_path,
                        config.as_ref().map(|c| c.name.as_str()),
                        Some(&device.id),
                    );

                    UpdateResult::action(UpdateAction::SpawnSession {
                        session_id,
                        device,
                        config: config.map(Box::new),
                    })
                }
                Err(e) => {
                    // Session creation failed
                    state.clear_loading();
                    if let Some(session) = state.session_manager.selected_mut() {
                        session.session.log_error(
                            LogSource::App,
                            format!("Failed to create session: {}", e),
                        );
                    }
                    UpdateResult::none()
                }
            }
        }
        Err(error_msg) => {
            // Device discovery failed, show startup dialog with error
            state.clear_loading();
            let configs = crate::config::load_all_configs(&state.project_path);
            state.show_startup_dialog(configs);
            state.startup_dialog_state.set_error(error_msg);
            UpdateResult::none()
        }
    }
}
```

### Import Requirements

Ensure these imports are present at the top of `update.rs`:
```rust
use crate::app::message::AutoLaunchSuccess;
use crate::config::LoadedConfigs;
```

### Handler Organization

Place the new handlers near related startup/device handlers. Suggested location: after `Message::ShowStartupDialog` handler (around line 1295).

### Acceptance Criteria

1. `Message::StartAutoLaunch` handler sets loading state and returns action
2. `Message::AutoLaunchProgress` handler updates loading message
3. `Message::AutoLaunchResult` handler creates session on success, shows dialog on error
4. All handlers use `UpdateResult` correctly
5. No dead code warnings
6. `cargo check` passes
7. `cargo clippy -- -D warnings` passes

### Testing

Add basic unit tests for the handlers:

```rust
#[test]
fn test_start_auto_launch_sets_loading() {
    let mut state = AppState::new();
    let configs = LoadedConfigs::default(); // or mock

    let result = update(&mut state, Message::StartAutoLaunch { configs });

    assert!(state.loading_state.is_some());
    assert_eq!(state.ui_mode, UiMode::Loading);
    assert!(matches!(result.action, Some(UpdateAction::DiscoverDevicesAndAutoLaunch { .. })));
}

#[test]
fn test_auto_launch_progress_updates_message() {
    let mut state = AppState::new();
    state.set_loading_phase("Initial");

    let _ = update(&mut state, Message::AutoLaunchProgress {
        message: "Detecting devices...".to_string()
    });

    assert_eq!(state.loading_state.as_ref().unwrap().message, "Detecting devices...");
}
```

### Notes

- The `AutoLaunchResult` handler reuses logic similar to existing `DeviceSelected` handler
- Error path shows `StartupDialog` which allows manual device selection
- The `save_last_selection` call ensures next startup remembers the choice
- Handler tests should go in `src/app/handler/tests.rs`

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
