//! NewSessionDialog - Unified session launch dialog
//!
//! Replaces DeviceSelector and StartupDialog with a single dialog featuring:
//! - Target Selector (left pane): Connected/Bootable device tabs
//! - Launch Context (right pane): Config, mode, flavor, dart-defines
//! - Fuzzy search modals for config/flavor selection
//! - Dart defines master-detail modal

mod dart_defines_modal;
mod device_groups;
mod device_list;
mod fuzzy_modal;
mod state;
mod tab_bar;
mod target_selector;

pub use dart_defines_modal::*;
pub use device_groups::*;
pub use device_list::*;
pub use fuzzy_modal::*;
pub use state::*;
pub use tab_bar::*;
pub use target_selector::*;

// Widget implementation comes in Phase 3-4
