//! Custom widget components

pub mod confirm_dialog;
pub mod devtools;
pub mod flutter_version_panel;
pub mod header;
pub mod log_view;
pub mod modal_overlay;
pub mod new_session_dialog;
mod search_input;
pub mod settings_panel;
pub mod shimmer;
mod tabs;
pub mod tag_filter;

pub use confirm_dialog::ConfirmDialog;
pub use devtools::{DevToolsView, PerformancePanel, WidgetInspector};
pub use flutter_version_panel::FlutterVersionPanel;
pub use header::MainHeader;
pub use log_view::{LogView, StatusInfo};
pub use new_session_dialog::*;
pub use search_input::SearchInput;
pub use settings_panel::SettingsPanel;
pub use shimmer::{lerp_color, shimmer_phase, shimmer_spans};
pub use tabs::SessionTabs;
pub use tag_filter::{render_tag_filter, render_tag_filter_with_regions};

// Re-export state types from app layer (these are used by render/)
pub use fdemon_app::confirm_dialog::ConfirmDialogState;
pub use fdemon_app::log_view_state::LogViewState;

// Re-export MouseCtx for use by widget modules (header.rs, tabs.rs) in Phase 3
// Tasks 06 and 07 import this from here rather than from render::mod directly.
pub use crate::render::MouseCtx;
