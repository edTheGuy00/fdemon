# Task: Deprecate Device Selector

**Objective**: Update device selector usage so it's only used for adding sessions to a running app, not for initial startup. Initial startup uses the new StartupDialog.

**Depends on**: Task 03 (Startup Dialog Widget), Task 06 (Startup Flow)

## Scope

- `src/app/state.rs` — Clarify device selector vs startup dialog usage
- `src/app/handler/keys.rs` — Update 'd' key to show appropriate dialog
- `src/tui/startup.rs` — Remove device selector from startup path
- `src/tui/widgets/device_selector.rs` — Add deprecation doc comment

## Details

### Usage Clarification

| Scenario | Previous | New |
|----------|----------|-----|
| Initial startup (auto_start=false) | DeviceSelector | StartupDialog |
| Initial startup (auto_start=true, no config) | DeviceSelector | StartupDialog (fallback) |
| Add session ('d' key, sessions running) | DeviceSelector | DeviceSelector (keep) |
| Add session ('d' key, no sessions) | DeviceSelector | StartupDialog |

### Key Handler Update

Update `'d'` key handling in `src/app/handler/keys.rs`:

```rust
// In handle_key_normal() or handle_key()

KeyCode::Char('d') => {
    // 'd' key: Add new session
    if state.has_running_sessions() {
        // Quick add: just show device selector (user already has config context)
        Some(Message::ShowDeviceSelector)
    } else {
        // No sessions: show full startup dialog
        Some(Message::ShowStartupDialog)
    }
}
```

### Startup Flow Update

In `src/tui/startup.rs`, the startup dialog should be shown instead of device selector when:
- `auto_start = false`
- `auto_start = true` but no valid config/device found

```rust
// In startup_flutter(), replace device selector fallbacks with startup dialog:

// Instead of:
state.ui_mode = UiMode::DeviceSelector;
state.device_selector.show_loading();

// Use:
let configs = load_all_configs(&state.project_path);
state.show_startup_dialog(configs);
```

### DeviceSelector Documentation

Add deprecation notice to `src/tui/widgets/device_selector.rs`:

```rust
//! Device selector modal widget
//!
//! **Note**: For initial app startup, prefer using `StartupDialog` which provides
//! a more comprehensive launch experience with config selection. DeviceSelector
//! is retained for the "add session" use case when sessions are already running.
//!
//! Use cases:
//! - Adding additional device session to running app ('d' key when sessions exist)
//!
//! Do NOT use for:
//! - Initial app startup (use StartupDialog instead)
```

### State Transitions

```
App Launch
    │
    ▼
auto_start?
    │
    ├─Yes─▶ Try auto-launch
    │           │
    │           ├─Success─▶ UiMode::Normal
    │           │
    │           └─Fail─▶ UiMode::StartupDialog (NOT DeviceSelector)
    │
    └─No──▶ UiMode::StartupDialog (NOT DeviceSelector)


During App (sessions running)
    │
    ▼
'd' key pressed
    │
    ▼
has_running_sessions?
    │
    ├─Yes─▶ UiMode::DeviceSelector (quick add)
    │
    └─No──▶ UiMode::StartupDialog (full experience)
```

### Message Flow Update

Update device selection message handling to only apply config from existing session context:

```rust
// In handler for DeviceSelected when adding to running app
Message::DeviceSelected { device } => {
    if state.has_running_sessions() {
        // Copy config from current session (if any) for new session
        let config = state.session_manager.selected()
            .and_then(|s| s.config.clone());

        // Create session with same config (different device)
        let result = if let Some(cfg) = config {
            state.session_manager.create_session_with_config(&device, cfg)
        } else {
            state.session_manager.create_session(&device)
        };

        // ... spawn session
    }
}
```

## Acceptance Criteria

1. Initial startup shows StartupDialog, not DeviceSelector
2. 'd' key shows DeviceSelector when sessions running
3. 'd' key shows StartupDialog when no sessions running
4. DeviceSelector still works for add-session use case
5. Deprecation notice added to DeviceSelector module

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_d_key_with_running_sessions() {
        let mut state = AppState::new();
        // Simulate running session
        state.session_manager.create_session(&test_device()).unwrap();

        let msg = handle_key(key('d'), &mut state);

        assert!(matches!(msg, Some(Message::ShowDeviceSelector)));
    }

    #[test]
    fn test_d_key_without_sessions() {
        let mut state = AppState::new();
        // No running sessions

        let msg = handle_key(key('d'), &mut state);

        assert!(matches!(msg, Some(Message::ShowStartupDialog)));
    }

    #[test]
    fn test_startup_shows_dialog_not_selector() {
        let mut state = AppState::new();
        let settings = Settings {
            behavior: BehaviorSettings { auto_start: false, ..Default::default() },
            ..Default::default()
        };

        // Simulate startup
        // After startup_flutter(), state should be StartupDialog, not DeviceSelector
        assert_eq!(state.ui_mode, UiMode::StartupDialog);
    }
}
```

## Notes

- DeviceSelector is NOT being removed, just its usage is being refined
- Emulator launch options move to StartupDialog for initial startup
- DeviceSelector keeps emulator options for add-session scenario
- Could consolidate to single dialog in future, but keep both for now

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (none yet)

**Implementation Details:**
(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo check` - Pending
- `cargo clippy -- -D warnings` - Pending
- `cargo test` - Pending
