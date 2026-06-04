//! State types for the Install Wizard panel.
//!
//! The wizard is opened when fdemon detects a missing or broken toolchain.
//! It shows a read-only preflight report (from `fdemon_daemon::toolchain`)
//! grouped into five ordered UI steps with roll-up status indicators.

use std::cell::Cell;

use fdemon_daemon::toolchain::{
    ComponentCheck, ComponentKind, ComponentStatus, HostPlatform, ToolchainReport,
};

use super::types::{
    GuidedCommand, StepExecStatus, StepExecution, StepStatus, WizardPane, WizardStepKind,
    MAX_LOG_TAIL,
};

/// A single UI step in the install wizard, grouping one or more component checks.
#[derive(Debug, Clone)]
pub struct WizardStep {
    /// Which wizard step this represents.
    pub kind: WizardStepKind,
    /// Human-readable title shown in the step list.
    pub title: String,
    /// Rolled-up status derived from the underlying component checks.
    pub status: StepStatus,
    /// Component checks rolled into this step (rendered in the detail pane).
    pub components: Vec<ComponentCheck>,
    /// Guided (copy-paste) commands for this step.
    ///
    /// Empty for steps that have no privileged/GUI actions. Populated by
    /// `build_steps()` when a required component is not `Ok`. Phase 3 uses
    /// this for the JDK install command on the `AndroidTools` step.
    pub guided_commands: Vec<GuidedCommand>,
}

/// Top-level state for the Install Wizard panel.
///
/// Owned by `AppState`, initialized via `InstallWizardState::opening()` when
/// the wizard is opened, and reset to `default()` when closed.
#[derive(Default)]
pub struct InstallWizardState {
    /// Whether the wizard panel is currently visible.
    pub visible: bool,
    /// Which pane has keyboard focus.
    pub focused_pane: WizardPane,
    /// Ordered list of UI steps populated by `apply_report`.
    pub steps: Vec<WizardStep>,
    /// Currently selected step index in the step list.
    pub selected_index: usize,
    /// Detail-pane vertical scroll offset (includes embedded doctor view).
    pub detail_scroll: usize,
    /// The full preflight report; `None` until `apply_report` is called.
    pub report: Option<ToolchainReport>,
    /// True while a preflight task is in-flight (initial open or `r` re-run).
    pub loading: bool,
    /// Status message shown at the bottom of the panel (e.g., error details).
    pub status_message: Option<String>,
    /// Render-hint: detail-pane visible height from the last rendered frame.
    ///
    /// Follows the `Cell<usize>` render-hint pattern (see docs/CODE_STANDARDS.md
    /// Principle 3). Defaults to 0, which signals "not yet rendered — use
    /// fallback". Written by the renderer; not mutated by message handlers.
    pub last_known_visible_height: Cell<usize>,
    /// Execution state for step runs (Phase 2+). Idle when nothing is running.
    ///
    /// Separate from the per-step `StepStatus` rollup (which reflects preflight
    /// results). Updated by the lifecycle mutators on `InstallWizardState`.
    pub execution: StepExecution,
    /// SDK path stashed after a successful `FlutterSdk` step execution.
    ///
    /// Set by the `WizardStepCompleted { kind: FlutterSdk, sdk_path: Some(p) }` handler
    /// so that the subsequent `PathConfig` step can resolve the Flutter `bin/` directory
    /// without re-running a preflight. Cleared when the wizard is closed.
    pub installed_sdk_path: Option<std::path::PathBuf>,
}

impl InstallWizardState {
    /// Fresh state for opening the wizard; preflight has not completed yet.
    ///
    /// Sets `visible = true` and `loading = true` so the TUI can show a
    /// spinner while the preflight task runs.
    pub fn opening() -> Self {
        Self {
            visible: true,
            loading: true,
            ..Self::default()
        }
    }

    /// Populate steps from a completed preflight report.
    ///
    /// Replaces any existing steps, clears `loading`, and clamps
    /// `selected_index` if the new step list is shorter.
    pub fn apply_report(&mut self, report: ToolchainReport) {
        self.steps = build_steps(&report);
        self.report = Some(report);
        self.loading = false;
        if self.selected_index >= self.steps.len() {
            self.selected_index = 0;
        }
    }

    /// Return the currently selected step, or `None` if the list is empty.
    pub fn selected_step(&self) -> Option<&WizardStep> {
        self.steps.get(self.selected_index)
    }

    /// The guided command the `c` key should copy: the first guided command of the
    /// currently selected step, if any.
    ///
    /// Returns `None` when the step list is empty or the selected step has no
    /// guided commands. Intended for use by the key handler that copies to the
    /// system clipboard.
    pub fn selected_guided_command(&self) -> Option<&GuidedCommand> {
        self.steps.get(self.selected_index)?.guided_commands.first()
    }

    /// Whether a step is currently executing.
    ///
    /// Returns `true` only when `execution.status == Running`. Used by handlers
    /// to guard against concurrent step runs.
    pub fn is_step_running(&self) -> bool {
        self.execution.status == StepExecStatus::Running
    }

    /// Begin a run: set `Running`, clear prior progress/log/summary, and record
    /// the step kind.
    ///
    /// Called by task 09's handlers when a step execution starts. Resets all
    /// progress fields so the TUI always shows fresh state.
    pub fn begin_step(&mut self, kind: WizardStepKind) {
        self.execution = StepExecution {
            kind: Some(kind),
            status: StepExecStatus::Running,
            phase_label: None,
            received: 0,
            total: None,
            log_tail: std::collections::VecDeque::new(),
            result_summary: None,
        };
    }

    /// Record a streamed log line, bounded to [`MAX_LOG_TAIL`] lines.
    ///
    /// When the tail is already at capacity, the oldest line is dropped via
    /// [`VecDeque::pop_front`] (O(1)) before the new line is appended via
    /// [`VecDeque::push_back`] (O(1)).
    pub fn push_step_log(&mut self, line: String) {
        if self.execution.log_tail.len() >= MAX_LOG_TAIL {
            self.execution.log_tail.pop_front();
        }
        self.execution.log_tail.push_back(line);
    }

    /// Update the download progress counters without disturbing the log tail.
    ///
    /// `received` is the number of bytes (or abstract units) transferred so far.
    /// `total` is `None` when the content length is unknown (e.g. chunked transfer).
    pub fn set_step_progress(&mut self, received: u64, total: Option<u64>) {
        self.execution.received = received;
        self.execution.total = total;
    }

    /// Update the current phase label (e.g. `"Cloning"`, `"Downloading"`,
    /// `"Precaching"`) without disturbing the log tail or progress counters.
    pub fn set_step_phase(&mut self, label: String) {
        self.execution.phase_label = Some(label);
    }

    /// Finish a run with a terminal status and a human-readable summary.
    ///
    /// `status` must be `Succeeded` or `Failed`; passing `Running` or `Idle`
    /// is a logic error but will not panic (the summary is still stored).
    /// After this call, [`is_step_running`][Self::is_step_running] returns `false`.
    pub fn finish_step(&mut self, status: StepExecStatus, summary: String) {
        self.execution.status = status;
        self.execution.result_summary = Some(summary);
    }
}

impl std::fmt::Debug for InstallWizardState {
    /// Manual `Debug` impl so `last_known_visible_height` shows its current
    /// value rather than the internal `Cell` representation.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallWizardState")
            .field("visible", &self.visible)
            .field("focused_pane", &self.focused_pane)
            .field("steps", &self.steps)
            .field("selected_index", &self.selected_index)
            .field("detail_scroll", &self.detail_scroll)
            .field("report", &self.report)
            .field("loading", &self.loading)
            .field("status_message", &self.status_message)
            .field(
                "last_known_visible_height",
                &self.last_known_visible_height.get(),
            )
            .field("execution", &self.execution)
            .field("installed_sdk_path", &self.installed_sdk_path)
            .finish()
    }
}

/// Roll up the status of a slice of component checks into a single `StepStatus`.
///
/// Rules (in priority order):
/// 1. Any `Missing` → `StepStatus::Missing`
/// 2. Any `Partial` or `Error` → `StepStatus::Partial`
/// 3. All `Ok` → `StepStatus::Ok`
/// 4. Empty slice → `StepStatus::Ok` (informational/no-component step)
fn rollup_status(components: &[ComponentCheck]) -> StepStatus {
    if components.is_empty() {
        return StepStatus::Ok;
    }
    let mut any_partial = false;
    for c in components {
        match c.status {
            ComponentStatus::Missing => return StepStatus::Missing,
            ComponentStatus::Partial | ComponentStatus::Error => any_partial = true,
            ComponentStatus::Ok | ComponentStatus::Unknown => {}
        }
    }
    if any_partial {
        StepStatus::Partial
    } else {
        StepStatus::Ok
    }
}

/// Per-OS guided command to install a JDK 17.
///
/// Privileged/GUI step — the wizard never auto-runs this command. It is shown
/// to the user to copy/paste. Lives in app-land (display concern) rather than
/// in the daemon's `jdk.rs` (which only handles `resolve_jdk_home` /
/// `configure_flutter_jdk_dir`).
fn jdk_guided_command(platform: HostPlatform) -> GuidedCommand {
    let (command, note) = match platform {
        HostPlatform::Linux => (
            "sudo apt install openjdk-17-jdk",
            Some("or: sudo dnf install java-17-openjdk-devel"),
        ),
        HostPlatform::MacOs => ("brew install openjdk@17", None),
        HostPlatform::Windows => ("winget install --id EclipseAdoptium.Temurin.17.JDK", None),
        HostPlatform::Unknown => ("Install a JDK 17 from https://adoptium.net", None),
    };
    GuidedCommand {
        label: "Install JDK 17".into(),
        command: command.into(),
        note: note.map(Into::into),
    }
}

/// Map a [`ToolchainReport`]'s components into the five ordered UI steps.
///
/// Step order: Prerequisites → AndroidTools → PathConfig → FlutterSdk → Doctor
///
/// Component grouping:
/// - `Prerequisites` — `ComponentKind::Prerequisites`, `ComponentKind::Git`
/// - `AndroidTools` — `AndroidCmdlineTools`, `AndroidPlatformTools`,
///   `AndroidPlatform`, `AndroidBuildTools`, `AndroidLicenses`, `Jdk`
/// - `PathConfig` — no components; status derived from whether Flutter is resolved
/// - `FlutterSdk` — `ComponentKind::FlutterSdk`
/// - `Doctor` — no components; detail comes from `report.doctor`
pub fn build_steps(report: &ToolchainReport) -> Vec<WizardStep> {
    let mut prerequisites: Vec<ComponentCheck> = Vec::new();
    let mut android_tools: Vec<ComponentCheck> = Vec::new();
    let mut flutter_sdk: Vec<ComponentCheck> = Vec::new();

    for check in &report.components {
        match check.kind {
            ComponentKind::Prerequisites | ComponentKind::Git => {
                prerequisites.push(check.clone());
            }
            ComponentKind::AndroidCmdlineTools
            | ComponentKind::AndroidPlatformTools
            | ComponentKind::AndroidPlatform
            | ComponentKind::AndroidBuildTools
            | ComponentKind::AndroidLicenses
            | ComponentKind::Jdk => {
                android_tools.push(check.clone());
            }
            ComponentKind::FlutterSdk => {
                flutter_sdk.push(check.clone());
            }
        }
    }

    // PathConfig status: Ok if Flutter is resolved (any FlutterSdk check is Ok),
    // Pending if not yet determined, Partial if partial.
    let path_config_status = if flutter_sdk.is_empty() {
        StepStatus::Pending
    } else {
        let flutter_ok = flutter_sdk.iter().any(|c| c.status == ComponentStatus::Ok);
        let flutter_partial = flutter_sdk
            .iter()
            .any(|c| matches!(c.status, ComponentStatus::Partial | ComponentStatus::Error));
        if flutter_ok {
            StepStatus::Ok
        } else if flutter_partial {
            StepStatus::Partial
        } else {
            StepStatus::Missing
        }
    };

    let prerequisites_status = rollup_status(&prerequisites);
    let android_status = rollup_status(&android_tools);
    let flutter_status = rollup_status(&flutter_sdk);

    // Doctor step: no components; always Ok when doctor data is present,
    // Pending when it is absent.
    let doctor_status = if report.doctor.is_some() {
        StepStatus::Ok
    } else {
        StepStatus::Pending
    };

    // Derive guided commands for the AndroidTools step: show the JDK install
    // command whenever the JDK component is not Ok (i.e. Missing/Partial/Error).
    // Derivation is pure — no I/O, no process spawning.
    let jdk_not_ok = android_tools
        .iter()
        .any(|c| c.kind == ComponentKind::Jdk && c.status != ComponentStatus::Ok);
    let android_guided: Vec<GuidedCommand> = if jdk_not_ok {
        vec![jdk_guided_command(report.platform.clone())]
    } else {
        Vec::new()
    };

    vec![
        WizardStep {
            kind: WizardStepKind::Prerequisites,
            title: "Prerequisites".to_string(),
            status: prerequisites_status,
            components: prerequisites,
            guided_commands: Vec::new(),
        },
        WizardStep {
            kind: WizardStepKind::AndroidTools,
            title: "Android Tools".to_string(),
            status: android_status,
            components: android_tools,
            guided_commands: android_guided,
        },
        WizardStep {
            kind: WizardStepKind::PathConfig,
            title: "PATH Configuration".to_string(),
            status: path_config_status,
            components: Vec::new(),
            guided_commands: Vec::new(),
        },
        WizardStep {
            kind: WizardStepKind::FlutterSdk,
            title: "Flutter SDK".to_string(),
            status: flutter_status,
            components: flutter_sdk,
            guided_commands: Vec::new(),
        },
        WizardStep {
            kind: WizardStepKind::Doctor,
            title: "Flutter Doctor".to_string(),
            status: doctor_status,
            components: Vec::new(),
            guided_commands: Vec::new(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use fdemon_daemon::toolchain::{
        ComponentCheck, ComponentKind, ComponentStatus, HostPlatform, HostShell, ToolchainReport,
    };

    use super::*;

    /// Build a minimal `ToolchainReport` for testing with the given components.
    fn make_report(components: Vec<ComponentCheck>) -> ToolchainReport {
        ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components,
            doctor: None,
        }
    }

    fn make_check(kind: ComponentKind, status: ComponentStatus) -> ComponentCheck {
        ComponentCheck {
            kind,
            status,
            detail: String::new(),
        }
    }

    #[test]
    fn test_opening_state_is_visible_and_loading() {
        let s = InstallWizardState::opening();
        assert!(s.visible);
        assert!(s.loading);
        assert!(s.steps.is_empty());
    }

    #[test]
    fn test_default_state_is_not_visible_and_not_loading() {
        let s = InstallWizardState::default();
        assert!(!s.visible);
        assert!(!s.loading);
        assert!(s.steps.is_empty());
    }

    #[test]
    fn test_build_steps_produces_five_ordered_steps() {
        let report = make_report(vec![
            make_check(ComponentKind::FlutterSdk, ComponentStatus::Ok),
            make_check(ComponentKind::Git, ComponentStatus::Ok),
            make_check(ComponentKind::Jdk, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidCmdlineTools, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidPlatformTools, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidPlatform, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidBuildTools, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidLicenses, ComponentStatus::Ok),
            make_check(ComponentKind::Prerequisites, ComponentStatus::Ok),
        ]);

        let steps = build_steps(&report);
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[0].kind, WizardStepKind::Prerequisites);
        assert_eq!(steps[1].kind, WizardStepKind::AndroidTools);
        assert_eq!(steps[2].kind, WizardStepKind::PathConfig);
        assert_eq!(steps[3].kind, WizardStepKind::FlutterSdk);
        assert_eq!(steps[4].kind, WizardStepKind::Doctor);
    }

    #[test]
    fn test_step_status_rollup_missing_wins() {
        let report = make_report(vec![
            make_check(ComponentKind::FlutterSdk, ComponentStatus::Ok),
            make_check(ComponentKind::Git, ComponentStatus::Missing),
            make_check(ComponentKind::Prerequisites, ComponentStatus::Ok),
        ]);

        let steps = build_steps(&report);
        let prereq_step = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::Prerequisites)
            .expect("Prerequisites step must exist");
        assert_eq!(
            prereq_step.status,
            StepStatus::Missing,
            "One Missing child should roll up to Missing"
        );
    }

    #[test]
    fn test_step_status_rollup_all_ok() {
        let report = make_report(vec![
            make_check(ComponentKind::Prerequisites, ComponentStatus::Ok),
            make_check(ComponentKind::Git, ComponentStatus::Ok),
        ]);

        let steps = build_steps(&report);
        let prereq_step = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::Prerequisites)
            .expect("Prerequisites step must exist");
        assert_eq!(
            prereq_step.status,
            StepStatus::Ok,
            "All-Ok children should roll up to Ok"
        );
    }

    #[test]
    fn test_step_status_rollup_partial_wins_over_ok() {
        let report = make_report(vec![
            make_check(ComponentKind::AndroidCmdlineTools, ComponentStatus::Ok),
            make_check(
                ComponentKind::AndroidPlatformTools,
                ComponentStatus::Partial,
            ),
            make_check(ComponentKind::AndroidPlatform, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidBuildTools, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidLicenses, ComponentStatus::Ok),
            make_check(ComponentKind::Jdk, ComponentStatus::Ok),
        ]);

        let steps = build_steps(&report);
        let android_step = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .expect("AndroidTools step must exist");
        assert_eq!(android_step.status, StepStatus::Partial);
    }

    #[test]
    fn test_apply_report_clears_loading_and_builds_steps() {
        let mut state = InstallWizardState::opening();
        assert!(state.loading);

        let report = make_report(vec![make_check(
            ComponentKind::FlutterSdk,
            ComponentStatus::Ok,
        )]);
        state.apply_report(report);

        assert!(!state.loading);
        assert_eq!(state.steps.len(), 5);
        assert!(state.report.is_some());
    }

    #[test]
    fn test_apply_report_clamps_selected_index() {
        let mut state = InstallWizardState {
            selected_index: 99,
            ..InstallWizardState::default()
        };

        let report = make_report(vec![make_check(
            ComponentKind::FlutterSdk,
            ComponentStatus::Ok,
        )]);
        state.apply_report(report);

        assert_eq!(
            state.selected_index, 0,
            "selected_index must be clamped when >= steps.len()"
        );
    }

    #[test]
    fn test_selected_step_returns_none_when_empty() {
        let state = InstallWizardState::default();
        assert!(state.selected_step().is_none());
    }

    #[test]
    fn test_selected_step_returns_correct_step() {
        let mut state = InstallWizardState::default();
        let report = make_report(vec![make_check(
            ComponentKind::FlutterSdk,
            ComponentStatus::Ok,
        )]);
        state.apply_report(report);
        state.selected_index = 0;
        assert_eq!(
            state.selected_step().map(|s| s.kind),
            Some(WizardStepKind::Prerequisites)
        );
    }

    #[test]
    fn test_render_hint_default_is_zero() {
        let state = InstallWizardState::default();
        assert_eq!(state.last_known_visible_height.get(), 0);
    }

    #[test]
    fn test_render_hint_can_be_set() {
        let state = InstallWizardState::default();
        // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md
        state.last_known_visible_height.set(24);
        assert_eq!(state.last_known_visible_height.get(), 24);
    }

    #[test]
    fn test_path_config_step_has_no_components() {
        let report = make_report(vec![make_check(
            ComponentKind::FlutterSdk,
            ComponentStatus::Ok,
        )]);
        let steps = build_steps(&report);
        let path_step = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::PathConfig)
            .expect("PathConfig step must exist");
        assert!(
            path_step.components.is_empty(),
            "PathConfig is informational and has no component checks"
        );
    }

    #[test]
    fn test_doctor_step_is_pending_when_no_doctor_output() {
        let report = make_report(vec![make_check(
            ComponentKind::FlutterSdk,
            ComponentStatus::Missing,
        )]);
        let steps = build_steps(&report);
        let doctor_step = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::Doctor)
            .expect("Doctor step must exist");
        assert_eq!(doctor_step.status, StepStatus::Pending);
    }

    #[test]
    fn test_flutter_sdk_components_grouped_correctly() {
        let report = make_report(vec![
            make_check(ComponentKind::FlutterSdk, ComponentStatus::Ok),
            make_check(ComponentKind::Prerequisites, ComponentStatus::Ok),
        ]);
        let steps = build_steps(&report);
        let flutter_step = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::FlutterSdk)
            .expect("FlutterSdk step must exist");
        assert_eq!(flutter_step.components.len(), 1);
        assert_eq!(flutter_step.components[0].kind, ComponentKind::FlutterSdk);
    }

    #[test]
    fn test_rollup_status_error_treated_as_partial() {
        let components = vec![
            make_check(ComponentKind::AndroidCmdlineTools, ComponentStatus::Ok),
            make_check(ComponentKind::Jdk, ComponentStatus::Error),
        ];
        let report = make_report(components);
        let steps = build_steps(&report);
        let android_step = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .expect("AndroidTools step must exist");
        assert_eq!(android_step.status, StepStatus::Partial);
    }

    // --- Execution state tests ---

    #[test]
    fn test_default_state_has_idle_execution() {
        let s = InstallWizardState::default();
        assert_eq!(s.execution.status, StepExecStatus::Idle);
        assert!(s.execution.log_tail.is_empty());
        assert!(s.execution.kind.is_none());
    }

    #[test]
    fn test_is_step_running_false_when_idle() {
        let s = InstallWizardState::default();
        assert!(!s.is_step_running());
    }

    #[test]
    fn test_begin_step_sets_running_and_clears() {
        let mut s = InstallWizardState::default();
        // Simulate leftover state from a previous run.
        s.execution.log_tail.push_back("old line".to_string());
        s.execution.received = 42;
        s.execution.result_summary = Some("old summary".to_string());
        s.execution.phase_label = Some("old phase".to_string());

        s.begin_step(WizardStepKind::FlutterSdk);

        assert_eq!(s.execution.status, StepExecStatus::Running);
        assert_eq!(s.execution.kind, Some(WizardStepKind::FlutterSdk));
        assert!(s.execution.log_tail.is_empty(), "log tail must be cleared");
        assert_eq!(s.execution.received, 0, "received must be reset");
        assert!(
            s.execution.result_summary.is_none(),
            "result_summary must be cleared"
        );
        assert!(
            s.execution.phase_label.is_none(),
            "phase_label must be cleared"
        );
        assert!(s.is_step_running());
    }

    #[test]
    fn test_log_tail_is_bounded() {
        let mut s = InstallWizardState::default();
        for i in 0..(MAX_LOG_TAIL + 50) {
            s.push_step_log(format!("line {i}"));
        }
        assert_eq!(
            s.execution.log_tail.len(),
            MAX_LOG_TAIL,
            "log tail must not exceed MAX_LOG_TAIL"
        );
        assert!(
            s.execution.log_tail.front().unwrap().contains("line 50"),
            "oldest lines must be dropped: first line should be 'line 50'"
        );
    }

    #[test]
    fn test_finish_step_sets_terminal_status_succeeded() {
        let mut s = InstallWizardState::default();
        s.begin_step(WizardStepKind::AndroidTools);
        assert!(s.is_step_running());

        s.finish_step(StepExecStatus::Succeeded, "All done".to_string());

        assert_eq!(s.execution.status, StepExecStatus::Succeeded);
        assert_eq!(s.execution.result_summary.as_deref(), Some("All done"));
        assert!(
            !s.is_step_running(),
            "is_step_running must be false after finish"
        );
    }

    #[test]
    fn test_finish_step_sets_terminal_status_failed() {
        let mut s = InstallWizardState::default();
        s.begin_step(WizardStepKind::Prerequisites);

        s.finish_step(StepExecStatus::Failed, "error: network timeout".to_string());

        assert_eq!(s.execution.status, StepExecStatus::Failed);
        assert!(s
            .execution
            .result_summary
            .as_deref()
            .unwrap()
            .contains("timeout"));
        assert!(!s.is_step_running());
    }

    #[test]
    fn test_progress_updates_do_not_touch_log() {
        let mut s = InstallWizardState::default();
        s.push_step_log("line 1".to_string());
        s.push_step_log("line 2".to_string());

        s.set_step_progress(1024, Some(4096));

        assert_eq!(s.execution.log_tail.len(), 2, "log tail must be untouched");
        assert_eq!(s.execution.received, 1024);
        assert_eq!(s.execution.total, Some(4096));
    }

    #[test]
    fn test_set_step_phase_does_not_touch_log_or_progress() {
        let mut s = InstallWizardState::default();
        s.push_step_log("log line".to_string());
        s.execution.received = 100;
        s.execution.total = Some(200);

        s.set_step_phase("Downloading".to_string());

        assert_eq!(s.execution.phase_label.as_deref(), Some("Downloading"));
        assert_eq!(s.execution.log_tail.len(), 1, "log tail must be untouched");
        assert_eq!(s.execution.received, 100, "received must be untouched");
        assert_eq!(s.execution.total, Some(200), "total must be untouched");
    }

    #[test]
    fn test_push_step_log_below_cap_retains_all_lines() {
        let mut s = InstallWizardState::default();
        for i in 0..10 {
            s.push_step_log(format!("line {i}"));
        }
        assert_eq!(s.execution.log_tail.len(), 10);
        assert_eq!(s.execution.log_tail[0], "line 0");
        assert_eq!(s.execution.log_tail[9], "line 9");
    }

    // --- Guided command tests ---

    fn report_with_jdk(status: ComponentStatus, platform: HostPlatform) -> ToolchainReport {
        ToolchainReport {
            platform,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::Jdk,
                status,
                detail: String::new(),
            }],
            doctor: None,
        }
    }

    #[test]
    fn test_android_step_has_jdk_guided_command_when_jdk_missing() {
        let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Linux);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        assert_eq!(android.guided_commands.len(), 1);
        assert!(android.guided_commands[0].command.contains("17"));
    }

    #[test]
    fn test_no_guided_command_when_jdk_ok() {
        let report = report_with_jdk(ComponentStatus::Ok, HostPlatform::Linux);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        assert!(android.guided_commands.is_empty());
    }

    #[test]
    fn test_android_step_has_jdk_guided_command_when_jdk_partial() {
        let report = report_with_jdk(ComponentStatus::Partial, HostPlatform::MacOs);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        assert_eq!(android.guided_commands.len(), 1);
        assert!(android.guided_commands[0].command.contains("brew"));
    }

    #[test]
    fn test_android_step_has_jdk_guided_command_when_jdk_error() {
        let report = report_with_jdk(ComponentStatus::Error, HostPlatform::Linux);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        assert_eq!(android.guided_commands.len(), 1);
    }

    #[test]
    fn test_jdk_command_linux_contains_apt() {
        let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Linux);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        let cmd = &android.guided_commands[0];
        assert_eq!(cmd.label, "Install JDK 17");
        assert!(cmd.command.contains("apt"));
        assert!(cmd.note.is_some(), "Linux should have an alternative note");
    }

    #[test]
    fn test_jdk_command_macos_uses_brew() {
        let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::MacOs);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        let cmd = &android.guided_commands[0];
        assert!(cmd.command.contains("brew"));
        assert!(cmd.note.is_none(), "macOS should have no alternative note");
    }

    #[test]
    fn test_jdk_command_windows_uses_winget() {
        let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Windows);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        assert!(android.guided_commands[0].command.contains("winget"));
    }

    #[test]
    fn test_jdk_command_unknown_platform_uses_adoptium() {
        let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Unknown);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        assert!(android.guided_commands[0].command.contains("adoptium.net"));
    }

    #[test]
    fn test_non_android_steps_have_no_guided_commands() {
        let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Linux);
        let steps = build_steps(&report);
        for step in &steps {
            if step.kind != WizardStepKind::AndroidTools {
                assert!(
                    step.guided_commands.is_empty(),
                    "Step {:?} should have no guided commands",
                    step.kind
                );
            }
        }
    }

    #[test]
    fn test_selected_guided_command_returns_none_when_no_steps() {
        let state = InstallWizardState::default();
        assert!(state.selected_guided_command().is_none());
    }

    #[test]
    fn test_selected_guided_command_returns_none_when_step_has_none() {
        let mut state = InstallWizardState::default();
        // Select Prerequisites step (index 0) — no guided commands
        let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Linux);
        state.apply_report(report);
        state.selected_index = 0; // Prerequisites
        assert!(state.selected_guided_command().is_none());
    }

    #[test]
    fn test_selected_guided_command_returns_first_when_android_selected() {
        let mut state = InstallWizardState::default();
        let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Linux);
        state.apply_report(report);
        // AndroidTools is index 1
        state.selected_index = 1;
        let cmd = state.selected_guided_command();
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().label, "Install JDK 17");
    }
}
