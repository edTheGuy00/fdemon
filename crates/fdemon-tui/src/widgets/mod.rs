//! Custom widget components

mod confirm_dialog;
pub mod devtools;
pub mod flutter_version_panel;
mod header;
mod log_view;
pub mod modal_overlay;
pub mod new_session_dialog;
mod search_input;
pub mod settings_panel;
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
pub use tabs::SessionTabs;
pub use tag_filter::render_tag_filter;

// Re-export state types from app layer (these are used by render/)
pub use fdemon_app::confirm_dialog::ConfirmDialogState;
pub use fdemon_app::log_view_state::LogViewState;

// Re-export MouseCtx for use by widget modules (header.rs, tabs.rs) in Phase 3
// Tasks 06 and 07 import this from here rather than from render::mod directly.
pub use crate::render::MouseCtx;

/// Convert a `ratatui::layout::Rect` to a `fdemon_app::MouseRect`.
///
/// A free function is used instead of `impl From<Rect> for MouseRect` because
/// `MouseRect` lives in `fdemon-app` and `Rect` lives in `ratatui`; implementing
/// a foreign trait for a foreign type from `fdemon-tui` (a third crate) would
/// violate Rust's orphan rule.
///
/// Call sites: `to_mouse_rect(area)`. Used by header.rs and tabs.rs in Phase 3
/// Tasks 06/07 to record clickable regions.
///
/// TODO(phase-3): Remove this allow when Tasks 06/07 (header/tab region
/// recording) add the first call site.
#[allow(dead_code)]
pub(crate) fn to_mouse_rect(r: ratatui::layout::Rect) -> fdemon_app::MouseRect {
    fdemon_app::MouseRect::new(r.x, r.y, r.width, r.height)
}
