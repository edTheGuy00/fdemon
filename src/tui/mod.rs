//! TUI presentation layer with signal handling
//!
//! This module provides the terminal user interface for Flutter Demon.
//! It is organized into focused submodules:
//!
//! - `runner`: Main entry points and event loop
//! - `actions`: Action dispatch and task execution
//! - `process`: Message processing with session routing
//! - `spawn`: Background task spawning
//! - `event`: Terminal event handling
//! - `editor`: Editor command execution (Phase 3)
//! - `hyperlinks`: File reference extraction for Link Highlight Mode
//! - `layout`: Layout calculation
//! - `render`: Frame rendering
//! - `selector`: Project/device selection
//! - `terminal`: Terminal setup/restore
//! - `widgets`: Reusable UI components

pub mod actions;
pub mod editor;
pub mod event;
pub mod hyperlinks;
pub mod layout;
pub mod process;
pub mod render;
pub mod runner;
pub mod selector;
pub mod spawn;
pub mod startup;
pub mod terminal;
pub mod widgets;

// Re-export main entry points
pub use runner::{run, run_with_project};
pub use selector::{select_project, SelectionResult};

// Re-export types used externally
pub use actions::SessionTaskMap;
pub use editor::{open_in_editor, EditorError, OpenResult};
pub use hyperlinks::FileReference;
