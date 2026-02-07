//! # fdemon-tui - Terminal UI
//!
//! Ratatui-based terminal interface for Flutter Demon. Creates an [`Engine`](fdemon_app::Engine)
//! and adds terminal rendering, event polling, and widget display.
//!
//! Depends on [`fdemon_core`] and [`fdemon_app`].
//!
//! ## Entry Points
//!
//! - [`run_with_project()`] - Main entry point: creates Engine, initializes terminal, runs event loop
//! - [`select_project()`] - Interactive project selector (when multiple Flutter projects found)
//!
//! ## Widgets
//!
//! Reusable TUI components in the [`widgets`] module:
//! - Log viewer with scrolling, filtering, and search
//! - Session tabs for multi-device management
//! - Settings panel with live editing
//! - Device selector modal
//! - Confirmation dialogs

pub(crate) mod event;
pub(crate) mod layout;
pub(crate) mod render;
pub mod runner;
pub mod selector;
pub(crate) mod startup;
pub(crate) mod terminal;
pub(crate) mod theme;
pub mod widgets;

#[cfg(test)]
pub mod test_utils;

// Re-export main entry points
pub use runner::run_with_project;
pub use selector::{select_project, SelectionResult};
