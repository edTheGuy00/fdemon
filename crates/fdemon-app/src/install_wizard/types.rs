//! Core types for the Install Wizard panel.

/// Which pane has keyboard focus in the Install Wizard panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WizardPane {
    /// Left pane: ordered step list
    #[default]
    StepList,
    /// Right pane: per-step detail and embedded doctor view
    Detail,
}

/// User-facing ordered steps (the install dependency order is handled later).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardStepKind {
    /// OS-level prerequisites (cmake, ninja, clang, etc. on Linux; Xcode on macOS).
    Prerequisites,
    /// Android SDK tools: cmdline-tools, platform-tools, platform, build-tools, licenses, JDK.
    AndroidTools,
    /// PATH and environment configuration (informational step; no component check).
    PathConfig,
    /// Flutter SDK detection and version.
    FlutterSdk,
    /// Embedded `flutter doctor -v` output summary.
    Doctor,
}

/// Per-step roll-up status derived from the underlying component checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    /// All components in this step are present and functional.
    Ok,
    /// Some components are present but degraded or incomplete.
    Partial,
    /// One or more required components are missing.
    Missing,
    /// Preflight has not yet completed for this step.
    Pending,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_pane_default_is_step_list() {
        assert_eq!(WizardPane::default(), WizardPane::StepList);
    }

    #[test]
    fn test_wizard_pane_variants_are_distinct() {
        assert_ne!(WizardPane::StepList, WizardPane::Detail);
    }

    #[test]
    fn test_step_status_variants_are_distinct() {
        assert_ne!(StepStatus::Ok, StepStatus::Missing);
        assert_ne!(StepStatus::Partial, StepStatus::Pending);
    }

    #[test]
    fn test_wizard_step_kind_copy() {
        let kind = WizardStepKind::FlutterSdk;
        let copy = kind;
        assert_eq!(kind, copy);
    }
}
