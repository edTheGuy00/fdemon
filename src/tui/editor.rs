//! Editor command execution for opening files at specific locations.
//!
//! This module re-exports editor functionality from the app layer.
//! All editor logic has been moved to `app/editor.rs` (Phase 1, Task 05).

// Re-export all types and functions from app layer
pub use crate::app::editor::*;
