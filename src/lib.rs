//! Flutter Demon Library
//!
//! A TUI application for managing Flutter development sessions.

// Module declarations
pub mod app;
pub mod common;
pub mod core;
pub mod daemon;
pub mod tui;

// Re-export main entry points
pub use app::{run, run_with_project};
