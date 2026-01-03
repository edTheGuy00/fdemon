## Task: Confirm Dialog UI

**Objective**: Implement the confirmation dialog widget and rendering for the quit confirmation flow, displaying a modal asking users to confirm quitting when sessions are running.

**Depends on**: Task 08 (Q key request quit flow)

---

### Scope

- `src/tui/render.rs`: Update `ConfirmDialog` rendering (currently stubbed)
- `src/tui/widgets/mod.rs`: Add `ConfirmDialog` widget export
- `src/tui/widgets/confirm_dialog.rs`: Create new widget file

---

### Current State

```rust
// In src/tui/render.rs - view function
match state.ui_mode {
    // ...
    UiMode::ConfirmDialog => {
        // TODO: Render confirmation dialog
        // For now, the normal view is shown
    }
    // ...
}
```

**Problem:** The confirmation dialog is stubbed - when `ui_mode` is `ConfirmDialog`, nothing special is rendered.

---

### Implementation Details

#### 1. Create ConfirmDialog Widget

```rust
// New file: src/tui/widgets/confirm_dialog.rs

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// State for the confirmation dialog
#[derive(Debug, Clone)]
pub struct ConfirmDialogState {
    /// The title of the dialog
    pub title: String,
    /// The message to display
    pub message: String,
    /// Number of running sessions (for display)
    pub session_count: usize,
}

impl ConfirmDialogState {
    pub fn quit_confirmation(session_count: usize) -> Self {
        Self {
            title: "Quit Flutter Demon?".to_string(),
            message: if session_count == 1 {
                "You have 1 running session.".to_string()
            } else {
                format!("You have {} running sessions.", session_count)
            },
            session_count,
        }
    }
}

/// Confirmation dialog widget
pub struct ConfirmDialog<'a> {
    state: &'a ConfirmDialogState,
}

impl<'a> ConfirmDialog<'a> {
    pub fn new(state: &'a ConfirmDialogState) -> Self {
        Self { state }
    }

    /// Calculate centered modal rect
    fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        Rect::new(x, y, width.min(area.width), height.min(area.height))
    }
}

impl Widget for ConfirmDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fixed modal size
        let modal_width = 50;
        let modal_height = 9;
        let modal_area = Self::centered_rect(modal_width, modal_height, area);

        // Clear the area behind the modal
        Clear.render(modal_area, buf);

        // Create the modal block with border
        let block = Block::default()
            .title(format!(" {} ", self.state.title))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .style(Style::default().bg(Color::DarkGray));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        // Layout: message + buttons
        let chunks = Layout::vertical([
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Message line 1
            Constraint::Length(1), // Message line 2
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Buttons
            Constraint::Min(0),    // Rest
        ])
        .split(inner);

        // Session count message
        let message = Paragraph::new(self.state.message.as_str())
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow));
        message.render(chunks[1], buf);

        // Warning message
        let warning = Paragraph::new("All Flutter processes will be terminated.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::White));
        warning.render(chunks[2], buf);

        // Buttons
        let buttons = Line::from(vec![
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled("y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("] Yes  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled("n", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("] No", Style::default().fg(Color::DarkGray)),
        ]);
        
        let buttons_para = Paragraph::new(buttons).alignment(Alignment::Center);
        buttons_para.render(chunks[4], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_confirm_dialog_state_single_session() {
        let state = ConfirmDialogState::quit_confirmation(1);
        assert!(state.message.contains("1 running session"));
    }

    #[test]
    fn test_confirm_dialog_state_multiple_sessions() {
        let state = ConfirmDialogState::quit_confirmation(3);
        assert!(state.message.contains("3 running sessions"));
    }

    #[test]
    fn test_confirm_dialog_rendering() {
        let state = ConfirmDialogState::quit_confirmation(2);
        let dialog = ConfirmDialog::new(&state);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();

        // Should contain dialog elements
        assert!(content.contains("Quit"));
        assert!(content.contains("2 running sessions"));
        assert!(content.contains("y"));
        assert!(content.contains("n"));
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let modal = ConfirmDialog::centered_rect(40, 10, area);

        // Should be centered
        assert_eq!(modal.x, 30); // (100 - 40) / 2
        assert_eq!(modal.y, 20); // (50 - 10) / 2
        assert_eq!(modal.width, 40);
        assert_eq!(modal.height, 10);
    }

    #[test]
    fn test_centered_rect_small_area() {
        let area = Rect::new(0, 0, 30, 8);
        let modal = ConfirmDialog::centered_rect(50, 10, area);

        // Should be clamped to area
        assert_eq!(modal.width, 30);
        assert_eq!(modal.height, 8);
    }
}
```

#### 2. Export from widgets/mod.rs

```rust
// In src/tui/widgets/mod.rs
mod confirm_dialog;
mod device_selector;
mod header;
mod log_view;
mod status_bar;
mod tabs;

pub use confirm_dialog::{ConfirmDialog, ConfirmDialogState};
pub use device_selector::{DeviceSelector, DeviceSelectorState};
pub use header::{Header, MainHeader};
pub use log_view::{LogView, LogViewState};
pub use status_bar::{StatusBar, StatusBarCompact};
pub use tabs::{HeaderWithTabs, SessionTabs};
```

#### 3. Update AppState with Dialog State

```rust
// In src/app/state.rs - add to AppState
pub struct AppState {
    // ... existing fields ...
    
    /// Confirmation dialog state
    pub confirm_dialog_state: Option<ConfirmDialogState>,
}

impl AppState {
    pub fn request_quit(&mut self) {
        if self.has_running_sessions() && self.settings.behavior.confirm_quit {
            // Create dialog state with session count
            let session_count = self.session_manager.running_sessions().len();
            self.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(session_count));
            self.ui_mode = UiMode::ConfirmDialog;
        } else {
            self.phase = AppPhase::Quitting;
        }
    }
    
    pub fn cancel_quit(&mut self) {
        self.confirm_dialog_state = None;
        self.ui_mode = UiMode::Normal;
    }
}
```

#### 4. Update render.rs

```rust
// In src/tui/render.rs - view function
pub fn view(frame: &mut Frame, state: &mut AppState) {
    let area = frame.area();
    let session_count = state.session_manager.len();
    let areas = layout::create_with_sessions(area, session_count);

    // ... render normal UI (header, tabs, logs, status) ...

    // Render modal overlays based on UI mode
    match state.ui_mode {
        UiMode::DeviceSelector | UiMode::Loading => {
            let selector = widgets::DeviceSelector::new(&state.device_selector);
            frame.render_widget(selector, area);
        }
        UiMode::ConfirmDialog => {
            // Render confirmation dialog
            if let Some(ref dialog_state) = state.confirm_dialog_state {
                let dialog = widgets::ConfirmDialog::new(dialog_state);
                frame.render_widget(dialog, area);
            }
        }
        UiMode::EmulatorSelector => {
            let selector = widgets::DeviceSelector::new(&state.device_selector);
            frame.render_widget(selector, area);
        }
        UiMode::Normal => {
            // No overlay
        }
    }
}
```

---

### Dialog Visual Design

```
┌────────────────────────────────────────────────┐
│           Quit Flutter Demon?                  │
├────────────────────────────────────────────────┤
│                                                │
│       You have 3 running sessions.             │
│   All Flutter processes will be terminated.   │
│                                                │
│            [y] Yes    [n] No                   │
│                                                │
└────────────────────────────────────────────────┘
```

### Colors

| Element | Color |
|---------|-------|
| Dialog background | DarkGray |
| Title | Default (white) |
| Session count message | Yellow |
| Warning message | White |
| [y] key | Green, Bold |
| [n] key | Red, Bold |
| Brackets | DarkGray |

---

### Acceptance Criteria

1. [ ] `ConfirmDialog` widget created in `widgets/confirm_dialog.rs`
2. [ ] Widget exported from `widgets/mod.rs`
3. [ ] `ConfirmDialogState` stores session count and messages
4. [ ] Dialog renders centered on screen
5. [ ] Dialog shows correct session count
6. [ ] Dialog shows [y] Yes and [n] No options
7. [ ] render.rs uses dialog state when `UiMode::ConfirmDialog`
8. [ ] AppState creates dialog state in `request_quit()`
9. [ ] AppState clears dialog state in `cancel_quit()`

---

### Testing

```rust
#[test]
fn test_dialog_shown_on_request_quit() {
    let mut state = AppState::new();
    state.settings.behavior.confirm_quit = true;
    
    // Create running sessions
    let device = test_device("d1", "iPhone");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.get_mut(id).unwrap().session.mark_started("app-1".into());
    
    update(&mut state, Message::RequestQuit);
    
    assert_eq!(state.ui_mode, UiMode::ConfirmDialog);
    assert!(state.confirm_dialog_state.is_some());
    assert_eq!(state.confirm_dialog_state.as_ref().unwrap().session_count, 1);
}

#[test]
fn test_dialog_cleared_on_cancel() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;
    state.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(2));
    
    update(&mut state, Message::CancelQuit);
    
    assert_eq!(state.ui_mode, UiMode::Normal);
    assert!(state.confirm_dialog_state.is_none());
}

#[test]
fn test_render_with_confirm_dialog() {
    use ratatui::{backend::TestBackend, Terminal};
    
    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;
    state.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(2));
    
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    
    terminal.draw(|f| render::view(f, &mut state)).unwrap();
    
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
    
    assert!(content.contains("Quit"));
    assert!(content.contains("2 running sessions"));
}
```

---

### Notes

- The dialog appears centered over the existing UI
- `Clear` widget is used to erase what's behind the modal
- Keep the dialog simple - just yes/no, no additional options
- The dialog state could be extended for other confirmations (e.g., close all sessions)
- Consider adding animation/highlighting for visual polish
- Dialog should be keyboard-only - no mouse support needed for now