//! Custom widget components

mod confirm_dialog;
mod header;
mod log_view;
pub mod new_session_dialog;
mod search_input;
mod settings_panel;
mod status_bar;
mod tabs;

pub use confirm_dialog::{ConfirmDialog, ConfirmDialogState};
pub use header::MainHeader;
pub use log_view::{LogView, LogViewState};
pub use new_session_dialog::*;
pub use search_input::SearchInput;
pub use settings_panel::SettingsPanel;
pub use status_bar::{StatusBar, StatusBarCompact};
pub use tabs::{HeaderWithTabs, SessionTabs};
