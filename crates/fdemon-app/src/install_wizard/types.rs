//! Core types for the Install Wizard panel.

use std::collections::VecDeque;

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

/// Execution status of a single wizard step run.
///
/// Tracks whether a step is idle, actively running, or has reached a terminal
/// state. Used by handlers to guard against concurrent runs and by the TUI to
/// render the appropriate progress indicator.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StepExecStatus {
    /// No run has started or the wizard was just opened.
    #[default]
    Idle,
    /// A step run is in progress.
    Running,
    /// The last run completed successfully.
    Succeeded,
    /// The last run encountered an error.
    Failed,
}

/// Maximum number of streamed log lines retained in [`StepExecution::log_tail`].
///
/// Derived from: 200 lines provides a meaningful tail for debugging without
/// unbounded memory growth during long-running installs.
pub const MAX_LOG_TAIL: usize = 200;

/// Live execution state for the step currently running (or last run).
///
/// Held on [`crate::install_wizard::InstallWizardState`] and updated by the
/// lifecycle mutators (`begin_step`, `push_step_log`, `set_step_progress`,
/// `set_step_phase`, `finish_step`). Separate from the per-step `StepStatus`
/// rollup (which reflects preflight results, not a live run).
#[derive(Debug, Clone, Default)]
pub struct StepExecution {
    /// Which step kind is (or was last) running; `None` when idle.
    pub kind: Option<WizardStepKind>,
    /// Whether the step is idle, running, succeeded, or failed.
    pub status: StepExecStatus,
    /// Current phase label streamed by the executor (e.g. "Cloning", "Downloading").
    pub phase_label: Option<String>,
    /// Number of bytes (or units) received so far in the current download.
    pub received: u64,
    /// Total expected bytes (or units); `None` when the total is unknown.
    pub total: Option<u64>,
    /// Bounded tail of streamed log lines (newest appended, oldest dropped at cap).
    ///
    /// Uses [`VecDeque`] to allow O(1) front-eviction when at capacity
    /// (via [`VecDeque::pop_front`]) instead of the O(n) `Vec::remove(0)`.
    pub log_tail: VecDeque<String>,
    /// Human-readable success summary or error message after the run finishes.
    pub result_summary: Option<String>,
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

    #[test]
    fn test_step_exec_status_default_is_idle() {
        assert_eq!(StepExecStatus::default(), StepExecStatus::Idle);
    }

    #[test]
    fn test_step_exec_status_variants_are_distinct() {
        assert_ne!(StepExecStatus::Idle, StepExecStatus::Running);
        assert_ne!(StepExecStatus::Succeeded, StepExecStatus::Failed);
    }

    #[test]
    fn test_step_execution_default_has_empty_log_tail() {
        let exec = StepExecution::default();
        assert!(exec.log_tail.is_empty());
        assert_eq!(exec.status, StepExecStatus::Idle);
        assert!(exec.kind.is_none());
        assert!(exec.phase_label.is_none());
        assert_eq!(exec.received, 0);
        assert!(exec.total.is_none());
        assert!(exec.result_summary.is_none());
    }

    #[test]
    fn test_max_log_tail_constant_is_200() {
        assert_eq!(MAX_LOG_TAIL, 200);
    }
}
