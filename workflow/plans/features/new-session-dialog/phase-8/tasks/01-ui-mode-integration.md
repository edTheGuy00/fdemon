# Task: UI Mode Integration

## Summary

Update the UiMode enum to use NewSessionDialog instead of separate StartupDialog and DeviceSelector modes. Wire up rendering for the new dialog.

## Files

| File | Action |
|------|--------|
| `src/app/state.rs` | Modify (update UiMode enum) |
| `src/tui/render/mod.rs` | Modify (update rendering) |

## Implementation

### 1. Update UiMode enum

```rust
// src/app/state.rs

/// UI display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
    /// Startup state - show NewSessionDialog
    #[default]
    Startup,

    /// Normal operation - showing log view
    Normal,

    /// NewSessionDialog is open (for adding sessions)
    NewSessionDialog,

    /// Help overlay is showing
    Help,

    /// Settings page
    Settings,

    /// Quit confirmation
    QuitConfirm,
}

// Remove old modes:
// - StartupDialog (replaced by NewSessionDialog)
// - DeviceSelector (replaced by NewSessionDialog)
```

### 2. Add NewSessionDialogState to AppState

```rust
// src/app/state.rs

use crate::tui::widgets::new_session_dialog::NewSessionDialogState;
use crate::daemon::ToolAvailability;

pub struct AppState {
    // ... existing fields ...

    /// New session dialog state (when dialog is open)
    pub new_session_dialog: Option<NewSessionDialogState>,

    /// Cached tool availability (checked at startup)
    pub tool_availability: ToolAvailability,

    // Remove these:
    // pub startup_dialog: Option<StartupDialogState>,
    // pub device_selector: DeviceSelectorState,
}

impl AppState {
    pub fn new(/* ... */) -> Self {
        Self {
            // ... existing fields ...
            new_session_dialog: None,
            tool_availability: ToolAvailability::default(),
        }
    }
}
```

### 3. Update rendering

```rust
// src/tui/render/mod.rs

use crate::tui::widgets::new_session_dialog::NewSessionDialog;

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    match state.ui_mode {
        UiMode::Startup | UiMode::NewSessionDialog => {
            // Render NewSessionDialog
            if let Some(ref dialog_state) = state.new_session_dialog {
                let dialog = NewSessionDialog::new(
                    dialog_state,
                    &state.tool_availability,
                );
                frame.render_widget(dialog, area);
            } else {
                // Should not happen, but handle gracefully
                render_loading(frame, area);
            }
        }

        UiMode::Normal => {
            // Render normal view (log view, status bar, etc.)
            render_normal_view(frame, state);

            // Render NewSessionDialog overlay if open
            if let Some(ref dialog_state) = state.new_session_dialog {
                let dialog = NewSessionDialog::new(
                    dialog_state,
                    &state.tool_availability,
                );
                frame.render_widget(dialog, area);
            }
        }

        UiMode::Help => {
            render_normal_view(frame, state);
            render_help_overlay(frame, state, area);
        }

        UiMode::Settings => {
            render_settings_page(frame, state);
        }

        UiMode::QuitConfirm => {
            render_normal_view(frame, state);
            render_quit_confirm(frame, area);
        }
    }
}

fn render_loading(frame: &mut Frame, area: Rect) {
    use ratatui::widgets::Paragraph;
    use ratatui::layout::Alignment;

    let loading = Paragraph::new("Loading...")
        .alignment(Alignment::Center);
    frame.render_widget(loading, area);
}
```

### 4. Remove old dialog rendering

```rust
// Remove these from render/mod.rs:

// fn render_startup_dialog(frame: &mut Frame, state: &AppState, area: Rect) { ... }
// fn render_device_selector(frame: &mut Frame, state: &AppState, area: Rect) { ... }
```

### 5. Update UiMode transitions

```rust
// src/app/handler/update.rs

/// Open NewSessionDialog for startup
fn transition_to_startup(state: &mut AppState) {
    let configs = state.loaded_configs.clone();
    state.new_session_dialog = Some(NewSessionDialogState::new(configs));
    state.ui_mode = UiMode::Startup;
}

/// Open NewSessionDialog to add device (from normal mode)
fn transition_to_new_session(state: &mut AppState) {
    let configs = state.loaded_configs.clone();
    state.new_session_dialog = Some(NewSessionDialogState::new(configs));
    state.ui_mode = UiMode::NewSessionDialog;
}

/// Close dialog and return to normal
fn transition_to_normal(state: &mut AppState) {
    state.new_session_dialog = None;
    state.ui_mode = UiMode::Normal;
}

/// After successful launch, close dialog
fn handle_launch_success(state: &mut AppState) {
    state.new_session_dialog = None;
    state.ui_mode = UiMode::Normal;
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_mode_default() {
        assert_eq!(UiMode::default(), UiMode::Startup);
    }

    #[test]
    fn test_transition_to_startup() {
        let mut state = create_test_state();

        transition_to_startup(&mut state);

        assert!(state.new_session_dialog.is_some());
        assert_eq!(state.ui_mode, UiMode::Startup);
    }

    #[test]
    fn test_transition_to_new_session() {
        let mut state = create_test_state_with_sessions();

        transition_to_new_session(&mut state);

        assert!(state.new_session_dialog.is_some());
        assert_eq!(state.ui_mode, UiMode::NewSessionDialog);
    }

    #[test]
    fn test_transition_to_normal() {
        let mut state = create_test_state_with_sessions();
        state.new_session_dialog = Some(NewSessionDialogState::new(LoadedConfigs::default()));

        transition_to_normal(&mut state);

        assert!(state.new_session_dialog.is_none());
        assert_eq!(state.ui_mode, UiMode::Normal);
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test ui_mode && cargo clippy -- -D warnings
```

## Notes

- UiMode::Startup and UiMode::NewSessionDialog both render NewSessionDialog
- The difference is context: Startup means no sessions yet
- NewSessionDialog overlay renders on top of normal view when adding device
- Old dialog types will be removed in task 03

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/src/app/state.rs` | Updated UiMode enum: Changed default to Startup, moved NewSessionDialog up, marked StartupDialog and DeviceSelector as legacy |
| `/Users/ed/Dev/zabin/flutter-demon/src/tui/render/mod.rs` | Updated rendering to handle Startup and NewSessionDialog modes with NewSessionDialog widget, removed unused centered_rect helper |
| `/Users/ed/Dev/zabin/flutter-demon/src/app/handler/keys.rs` | Updated handle_key to route Startup and NewSessionDialog modes to handle_key_new_session_dialog |

### Notable Decisions/Tradeoffs

1. **Kept legacy modes**: StartupDialog and DeviceSelector modes were marked as legacy but not removed yet per task instructions. This allows for gradual migration without breaking existing code paths.
2. **Combined mode handling**: Both UiMode::Startup and UiMode::NewSessionDialog use the same rendering logic and key handler, simplifying the implementation.
3. **Default mode changed**: UiMode::default() now returns Startup instead of Normal, aligning with the new startup flow.

### Testing Performed

- `cargo check` - Passed (no compilation errors)
- `cargo test --lib` - Passed (1559 tests passed, 0 failed)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Existing state fields not removed**: AppState still contains startup_dialog_state and device_selector fields. These will be removed in task 03 after handler migration is complete.
2. **Legacy mode paths still active**: Code paths using StartupDialog and DeviceSelector modes still exist and function. Task 02 will migrate handlers to use the new modes.
