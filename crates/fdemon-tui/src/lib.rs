//! fdemon-tui - Terminal UI for Flutter Demon
//!
//! This crate provides the ratatui-based terminal interface. It creates an Engine
//! from fdemon-app and adds terminal rendering, event polling, and widget display.

pub mod event;
pub mod layout;
pub mod render;
pub mod runner;
pub mod selector;
pub mod startup;
pub mod terminal;
pub mod widgets;

#[cfg(test)]
pub mod test_utils;

// Re-export main entry points
pub use runner::run_with_project;
pub use selector::{select_project, SelectionResult};
