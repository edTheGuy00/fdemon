//! TUI presentation layer
//!
//! This module provides the terminal user interface for Flutter Demon.
//! It is organized into focused submodules:
//!
//! - `runner`: Main entry points and event loop
//! - `event`: Terminal event handling
//! - `editor`: Editor command execution (Phase 3)
//! - `hyperlinks`: File reference extraction for Link Highlight Mode
//! - `layout`: Layout calculation
//! - `render`: Frame rendering
//! - `selector`: Project/device selection
//! - `startup`: Startup and cleanup logic
//! - `terminal`: Terminal setup/restore
//! - `widgets`: Reusable UI components
//! - `test_utils`: Test utilities for widget testing (test-only)

pub mod editor;
pub mod event;
pub mod hyperlinks;
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
pub use runner::{run, run_with_project};
pub use selector::{select_project, SelectionResult};

// Re-export types used externally
pub use editor::{open_in_editor, EditorError, OpenResult};
pub use hyperlinks::FileReference;
