//! Confirm dialog state.
//!
//! Data model for confirmation dialogs. The rendering widget
//! lives in tui/widgets/confirm_dialog.rs.

use crate::message::Message;

#[derive(Debug, Clone)]
pub struct ConfirmDialogState {
    pub title: String,
    pub message: String,
    pub session_count: usize,
    pub options: Vec<(String, Message)>,
    /// Optional warning text shown below the message line (rendered in a
    /// muted/primary colour). Set to `Some(...)` only when the action has
    /// irreversible side-effects (e.g. quitting terminates all Flutter
    /// processes). `None` suppresses the warning row entirely.
    pub warning: Option<String>,
}

impl ConfirmDialogState {
    /// Create a generic confirmation dialog.
    ///
    /// `warning` defaults to `None`; use
    /// [`ConfirmDialogState::with_warning`] or set the field directly when
    /// the action has side effects that deserve a prominent notice.
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        options: Vec<(&str, Message)>,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            session_count: 0,
            options: options
                .into_iter()
                .map(|(label, msg)| (label.to_string(), msg))
                .collect(),
            warning: None,
        }
    }

    /// Builder-style method to attach an optional warning string.
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warning = Some(warning.into());
        self
    }

    /// Create a quit confirmation dialog state.
    ///
    /// The warning "All Flutter processes will be terminated." is set because
    /// quitting terminates every running Flutter process.
    pub fn quit_confirmation(session_count: usize) -> Self {
        Self {
            title: "Quit Flutter Demon?".to_string(),
            message: if session_count == 1 {
                "You have 1 running session.".to_string()
            } else {
                format!("You have {} running sessions.", session_count)
            },
            session_count,
            options: vec![
                ("Quit".to_string(), Message::ConfirmQuit),
                ("Cancel".to_string(), Message::CancelQuit),
            ],
            warning: Some("All Flutter processes will be terminated.".to_string()),
        }
    }
}
