//! Confirm dialog state.
//!
//! Data model for confirmation dialogs. The rendering widget
//! lives in tui/widgets/confirm_dialog.rs.

use crate::app::message::Message;

#[derive(Debug, Clone)]
pub struct ConfirmDialogState {
    pub title: String,
    pub message: String,
    pub session_count: usize,
    pub options: Vec<(String, Message)>,
}

impl ConfirmDialogState {
    /// Create a generic confirmation dialog
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
        }
    }

    /// Create a quit confirmation dialog state
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
        }
    }
}
