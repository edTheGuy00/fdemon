//! Custom widget components

mod confirm_dialog;
pub mod devtools;
mod header;
mod log_view;
pub mod modal_overlay;
pub mod new_session_dialog;
mod search_input;
pub mod settings_panel;
mod tabs;

pub use confirm_dialog::ConfirmDialog;
pub use devtools::{DevToolsView, PerformancePanel, WidgetInspector};
pub use header::MainHeader;
pub use log_view::{LogView, StatusInfo};
pub use new_session_dialog::*;
pub use search_input::SearchInput;
pub use settings_panel::SettingsPanel;
pub use tabs::SessionTabs;

// Re-export state types from app layer (these are used by render/)
pub use fdemon_app::confirm_dialog::ConfirmDialogState;
pub use fdemon_app::log_view_state::LogViewState;
