//! NewSessionDialog state types (App layer)
//!
//! These types represent the Model in the TEA pattern for the NewSessionDialog feature.
//! They are owned by the App layer and used by the TUI layer for rendering.

mod state;
mod types;

// Re-export all types
pub use state::*;
pub use types::*;
