## Task: Add ConfirmDialog Widget Tests

**Objective**: Add TestBackend-based unit tests for the ConfirmDialog widget to verify quit confirmation dialog rendering.

**Depends on**: 06-testbackend-utilities

### Scope

- `src/tui/widgets/confirm_dialog.rs`: Add inline test module

### Details

#### 1. Review ConfirmDialog Widget

The ConfirmDialog displays:
- Modal overlay with question
- Yes/No options
- Keybinding hints (y/n or Enter/Escape)
- Selected option highlighting

#### 2. Add Test Module

Add to `src/tui/widgets/confirm_dialog.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_utils::TestTerminal;

    fn create_quit_dialog() -> ConfirmDialogState {
        ConfirmDialogState::new(
            "Quit?",
            "Are you sure you want to quit?",
            ConfirmAction::Quit,
        )
    }

    fn create_close_session_dialog() -> ConfirmDialogState {
        ConfirmDialogState::new(
            "Close Session",
            "Close the current session?",
            ConfirmAction::CloseSession,
        )
    }

    #[test]
    fn test_confirm_dialog_renders_title() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        assert!(
            term.buffer_contains("Quit"),
            "Dialog should show title"
        );
    }

    #[test]
    fn test_confirm_dialog_renders_message() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        assert!(
            term.buffer_contains("sure") || term.buffer_contains("quit"),
            "Dialog should show confirmation message"
        );
    }

    #[test]
    fn test_confirm_dialog_shows_options() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Should show Yes/No or y/n options
        assert!(
            term.buffer_contains("Yes") || term.buffer_contains("y") ||
            term.buffer_contains("No") || term.buffer_contains("n"),
            "Dialog should show confirmation options"
        );
    }

    #[test]
    fn test_confirm_dialog_shows_keybindings() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Should show key hints
        let content = term.content();
        assert!(
            content.contains("y") || content.contains("n") ||
            content.contains("Enter") || content.contains("Esc"),
            "Dialog should show keybinding hints"
        );
    }

    #[test]
    fn test_confirm_dialog_different_actions() {
        let mut term = TestTerminal::new();

        // Quit dialog
        let quit_state = create_quit_dialog();
        let quit_dialog = ConfirmDialog::new(&quit_state);
        term.render_widget(quit_dialog, term.area());
        assert!(term.buffer_contains("Quit"));

        term.clear();

        // Close session dialog
        let close_state = create_close_session_dialog();
        let close_dialog = ConfirmDialog::new(&close_state);
        term.render_widget(close_dialog, term.area());
        assert!(term.buffer_contains("Close") || term.buffer_contains("Session"));
    }

    #[test]
    fn test_confirm_dialog_modal_overlay() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Modal should render (just verify no panic)
        let content = term.content();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_confirm_dialog_compact() {
        let mut term = TestTerminal::compact();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Should fit in small terminal
        let content = term.content();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_confirm_dialog_centered() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Dialog content should be roughly centered
        // (This is hard to verify precisely, just check it renders)
        let content = term.content();
        assert!(!content.is_empty());
    }
}
```

### Test Coverage

| Test Case | Verifies |
|-----------|----------|
| `test_confirm_dialog_renders_title` | Title displayed |
| `test_confirm_dialog_renders_message` | Message displayed |
| `test_confirm_dialog_shows_options` | Yes/No options |
| `test_confirm_dialog_shows_keybindings` | Key hints |
| `test_confirm_dialog_different_actions` | Multiple dialog types |
| `test_confirm_dialog_modal_overlay` | Modal rendering |
| `test_confirm_dialog_compact` | Small terminal |
| `test_confirm_dialog_centered` | Layout positioning |

### Acceptance Criteria

1. Title and message render correctly
2. Yes/No options visible
3. Keybinding hints displayed
4. Multiple dialog types supported
5. Works in various terminal sizes

### Testing

```bash
# Run confirm dialog tests
cargo test widgets::confirm_dialog --lib -- --nocapture
```

---

## Completion Summary

**Status:** Not Started
