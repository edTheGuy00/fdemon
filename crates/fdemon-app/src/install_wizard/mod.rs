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

pub use state::*;
pub use types::*;
