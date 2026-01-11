## Task: Integrate with UiMode System

**Objective**: Add `UiMode::NewSessionDialog` and integrate with the render system.

**Depends on**: Task 04 (State transitions)

**Estimated Time**: 25 minutes

### Background

The application uses `UiMode` to track which UI state is active. The new dialog needs its own mode that coexists with the old modes during the transition period.

### Scope

- `src/app/state.rs`: Add `UiMode::NewSessionDialog` variant, add state field
- `src/tui/render/mod.rs`: Add placeholder rendering for new mode

### Changes Required

**Update `src/app/state.rs` UiMode enum:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
    #[default]
    Normal,
    DeviceSelector,      // Old - kept for transition
    StartupDialog,       // Old - kept for transition
    NewSessionDialog,    // New unified dialog
    Settings,
    EmulatorSelector,
    ConfirmDialog,
    Loading,
    Help,
}
```

**Add field to AppState:**

```rust
pub struct AppState {
    // ... existing fields ...

    /// New session dialog state (unified dialog)
    pub new_session_dialog_state: NewSessionDialogState,

    // Keep old fields during transition:
    // pub device_selector: DeviceSelectorState,
    // pub startup_dialog_state: StartupDialogState,
}
```

**Update AppState::new():**

```rust
impl AppState {
    pub fn new() -> Self {
        Self {
            // ... existing initializations ...
            new_session_dialog_state: NewSessionDialogState::new(),
        }
    }
}
```

**Add helper methods to AppState:**

```rust
impl AppState {
    // ... existing methods ...

    /// Show the new session dialog
    pub fn show_new_session_dialog(&mut self, configs: LoadedConfigs) {
        self.new_session_dialog_state = NewSessionDialogState::with_configs(configs);
        self.ui_mode = UiMode::NewSessionDialog;
    }

    /// Hide the new session dialog
    pub fn hide_new_session_dialog(&mut self) {
        self.ui_mode = UiMode::Normal;
    }

    /// Check if new session dialog is visible
    pub fn is_new_session_dialog_visible(&self) -> bool {
        self.ui_mode == UiMode::NewSessionDialog
    }
}
```

**Update `src/tui/render/mod.rs`:**

Add placeholder rendering in the main render function:

```rust
pub fn render(frame: &mut Frame, state: &AppState) {
    // ... existing code ...

    match state.ui_mode {
        // ... existing cases ...

        UiMode::NewSessionDialog => {
            // Render background (Normal mode)
            render_normal_mode(frame, state);

            // Placeholder for new dialog - will be implemented in Phase 3
            // For now, just show a placeholder block
            let area = frame.area();
            let modal_area = centered_rect(80, 70, area);

            frame.render_widget(Clear, modal_area);
            let block = Block::default()
                .title(" New Session (Coming Soon) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .style(Style::default().bg(Color::DarkGray));
            frame.render_widget(block, modal_area);
        }
    }
}

/// Calculate centered rect (reusable helper)
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
```

**Add import in render/mod.rs:**

```rust
use crate::tui::widgets::new_session_dialog::NewSessionDialogState;
```

### Acceptance Criteria

1. `UiMode::NewSessionDialog` variant added
2. `new_session_dialog_state` field added to `AppState`
3. Helper methods: `show_new_session_dialog()`, `hide_new_session_dialog()`, `is_new_session_dialog_visible()`
4. Placeholder rendering in render function
5. Old modes (`DeviceSelector`, `StartupDialog`) still work
6. `cargo check` passes
7. `cargo test` passes (existing tests shouldn't break)
8. `cargo clippy -- -D warnings` passes

### Testing

```rust
#[test]
fn test_new_session_dialog_visibility() {
    let mut state = AppState::new();
    assert!(!state.is_new_session_dialog_visible());

    state.show_new_session_dialog(LoadedConfigs::default());
    assert!(state.is_new_session_dialog_visible());
    assert_eq!(state.ui_mode, UiMode::NewSessionDialog);

    state.hide_new_session_dialog();
    assert!(!state.is_new_session_dialog_visible());
    assert_eq!(state.ui_mode, UiMode::Normal);
}
```

### Notes

- Old `DeviceSelector` and `StartupDialog` modes kept during transition
- Placeholder rendering shows visual feedback that the mode is recognized
- Actual widget rendering comes in Phase 3-4
- Helper methods follow existing patterns (`show_startup_dialog`, etc.)
