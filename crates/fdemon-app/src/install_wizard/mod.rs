//! # Install Wizard Panel State
//!
//! State and types for the Install Wizard panel, which guides users through
//! Flutter toolchain setup when a missing or broken toolchain is detected.
//!
//! This module mirrors the `flutter_version/` pattern:
//! - `state.rs`  — `InstallWizardState`, `WizardStep`, `build_steps()`
//! - `types.rs`  — `WizardPane`, `WizardStepKind`, `StepStatus`

mod state;
mod types;
pub mod version_picker;

pub(crate) use state::is_jdk_actionable;
pub use state::*;
pub use types::{
    GuidedCommand, StepExecStatus, StepExecution, StepStatus, WizardOrigin, WizardPane,
    WizardStepKind, MAX_LOG_TAIL,
};
pub use version_picker::{
    group_releases, PickerChannel, PickerFetch, PickerRow, VersionPickerState,
};

// Re-export the daemon toolchain *display* types so presentation-layer widgets can
// consume them without a direct fdemon-tui -> fdemon-daemon dependency.
// n6: extend the gateway to include all types needed by install-wizard TUI tests,
// so no module in fdemon-tui needs to import directly from fdemon_daemon::toolchain.
pub use fdemon_daemon::toolchain::{
    ComponentCheck, ComponentKind, ComponentStatus, DoctorLine, DoctorMarker, HostPlatform,
    HostShell, LinuxPackageManager, ToolchainReport,
};
