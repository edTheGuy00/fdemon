//! NewSessionDialog - Unified session launch dialog
//!
//! Replaces DeviceSelector and StartupDialog with a single dialog featuring:
//! - Target Selector (left pane): Connected/Bootable device tabs
//! - Launch Context (right pane): Config, mode, flavor, dart-defines
//! - Fuzzy search modals for config/flavor selection
//! - Dart defines master-detail modal

mod dart_defines_modal;
mod fuzzy_modal;
mod state;

pub use dart_defines_modal::*;
pub use fuzzy_modal::*;
pub use state::*;

// Widget implementation comes in Phase 3-4
