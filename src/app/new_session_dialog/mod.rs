//! NewSessionDialog state types (App layer)
//!
//! These types represent the Model in the TEA pattern for the NewSessionDialog feature.
//! They are owned by the App layer and used by the TUI layer for rendering.

pub mod device_groups;
pub mod fuzzy;
mod state;
pub mod target_selector_state;
mod types;

// Re-export all types
pub use device_groups::{DeviceListItem, GroupedBootableDevice};
pub use state::*;
pub use target_selector_state::TargetSelectorState;
pub use types::*;
