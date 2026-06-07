//! State types for the Install Wizard panel.
//!
//! The wizard is opened when fdemon detects a missing or broken toolchain.
//! It shows a read-only preflight report (from `fdemon_daemon::toolchain`)
//! grouped into five ordered UI steps with roll-up status indicators.

use std::cell::Cell;

use fdemon_daemon::toolchain::{
    parse_missing_prereq_keys, ComponentCheck, ComponentKind, ComponentStatus, HostPlatform,
    LinuxPackageManager, ToolchainReport, PREREQ_KEY_COCOAPODS, PREREQ_KEY_GIT, PREREQ_KEY_GLU,
    PREREQ_KEY_LIBSTDCPP, PREREQ_KEY_ROSETTA, PREREQ_KEY_XCODE_CLT,
};
use tokio_util::sync::CancellationToken;

use super::types::{
    GuidedCommand, StepExecStatus, StepExecution, StepStatus, WizardOrigin, WizardPane,
    WizardStepKind, MAX_LOG_TAIL,
};

/// A running install task: a `JoinHandle` paired with a `CancellationToken`.
///
/// Held on [`InstallWizardState::install_task`] while a wizard step is in
/// flight. Cleared (via `take()`) on completion, failure, or cancellation so
/// a stale handle never lingers.
///
/// **Lifecycle:** The token is minted and stored synchronously by
/// `handle_run_selected_step` (so `Esc` can cancel even before the
/// `WizardInstallTaskReady` message arrives). The `JoinHandle` starts as
/// `None` and is upgraded to `Some` when `handle_install_task_ready` validates
/// the `kind` + `run_seq` pair — ensuring late ready messages from a cancelled
/// or superseded run never clobber a live handle.
pub struct InstallTaskHandle {
    /// Token used to signal the install operation to stop.
    ///
    /// Call `cancel.cancel()` to set the token, which causes the running
    /// installer to return `Err(Error::Cancelled)` at the next poll point.
    ///
    /// Always `Some` — the token is minted synchronously by
    /// `handle_run_selected_step` before any async work begins.
    pub cancel: CancellationToken,
    /// The async task running the install operation.
    ///
    /// `None` until `handle_install_task_ready` upgrades it (after spawn
    /// returns inside `handle_action`). The join handle is only used as a
    /// backstop abort; the token is the primary cancellation mechanism.
    pub join: Option<tokio::task::JoinHandle<()>>,
}

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
/// Owned by `AppState`, initialized via `InstallWizardState::opening(WizardOrigin::UserInvoked)` when
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
    /// Index of the selected guided command within the selected step.
    ///
    /// Defaults to `0`. Advanced by `]` (`InstallWizardNextCommand`) and
    /// retreated by `[` (`InstallWizardPrevCommand`). Reset to `0` whenever
    /// the selected step changes (step list navigation or `apply_report`).
    pub selected_command_index: usize,
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

    /// Handle for the currently running install task (Phase 5, Task 03).
    ///
    /// The `CancellationToken` inside is minted synchronously by
    /// `handle_run_selected_step` and stored here **before** `RunWizardStep`
    /// is dispatched, so `Esc` can always fire the token even in the window
    /// before `WizardInstallTaskReady` arrives. The `JoinHandle` starts as
    /// `None` and is upgraded by `handle_install_task_ready` once the token's
    /// `kind` + `run_seq` are validated.
    ///
    /// Cleared (`take()`d) on completion, failure, or cancellation to prevent
    /// stale handles from lingering. `None` when no step is in flight.
    pub install_task: Option<InstallTaskHandle>,

    /// Monotonically increasing counter bumped by `begin_step` each time a new
    /// run starts. Carried by `WizardInstallTaskReady` so
    /// `handle_install_task_ready` can reject late-arriving ready messages from
    /// a previous run (cancel→retry scenario).
    pub run_seq: u64,

    /// One-shot guard that prevents device discovery from being dispatched more
    /// than once per wizard session (Phase 5, Task 04).
    ///
    /// Set to `true` the first time the handback path fires — either via the
    /// auto-close in `handle_preflight_completed` or via a manual Esc/HideInstallWizard
    /// with a live SDK.  Prevents a race where both auto-close and an immediately
    /// following Esc would each dispatch `DiscoverDevices`.
    ///
    /// Reset to `false` only when the wizard is fully re-opened via `opening()`.
    pub handback_done: bool,

    /// Why the wizard was opened. Gates the handback (see `close_wizard_and_dispatch_discovery`).
    ///
    /// `Bootstrap` — opened at startup because the toolchain was missing/broken;
    /// auto-advances to device discovery when healthy.
    /// `UserInvoked` — opened by the `I` key; informational only, never hands back.
    pub origin: WizardOrigin,
}

impl InstallWizardState {
    /// Fresh state for opening the wizard; preflight has not completed yet.
    ///
    /// Sets `visible = true` and `loading = true` so the TUI can show a
    /// spinner while the preflight task runs.  The `origin` parameter records
    /// why the wizard was opened, which gates the post-install handback.
    pub fn opening(origin: WizardOrigin) -> Self {
        Self {
            visible: true,
            loading: true,
            origin,
            ..Self::default()
        }
    }

    /// `true` when the wizard was opened to bootstrap a missing/broken toolchain.
    ///
    /// Only a `Bootstrap` origin auto-advances to device discovery after the
    /// toolchain becomes healthy.  A `UserInvoked` open never hands back.
    pub fn is_bootstrap(&self) -> bool {
        self.origin == WizardOrigin::Bootstrap
    }

    /// `true` when a report is present and every component is `Ok`.
    ///
    /// Drives the "All set" hint in the TUI header (task 02).  Returns `false`
    /// when no report has been applied yet.
    pub fn all_components_ok(&self) -> bool {
        self.report.as_ref().is_some_and(|r| {
            !r.components.is_empty() && r.components.iter().all(|c| c.status == ComponentStatus::Ok)
        })
    }

    /// Populate steps from a completed preflight report.
    ///
    /// Replaces any existing steps, clears `loading`, and clamps
    /// `selected_index` if the new step list is shorter.
    /// Also resets `selected_command_index` to 0 since the step list is rebuilt.
    ///
    /// **Execution reset (F-PR53-12):** clears `execution` back to `Idle` so
    /// that a stale `Failed`/`Cancelled`/`Succeeded` display state from a
    /// previous run does not mask the freshly rebuilt component list.  The
    /// handback predicate [`flutter_now_live`][Self::flutter_now_live] reads
    /// `report.components`, not `execution`, so clearing execution here does
    /// not affect auto-close behaviour.
    pub fn apply_report(&mut self, report: ToolchainReport) {
        self.steps = build_steps(&report);
        self.report = Some(report);
        self.loading = false;
        if self.selected_index >= self.steps.len() {
            self.selected_index = 0;
        }
        self.selected_command_index = 0;
        // Clear the per-run execution display so the refreshed component list
        // is shown rather than a stale progress/result view.
        self.execution = StepExecution::default();
    }

    /// Return the currently selected step, or `None` if the list is empty.
    pub fn selected_step(&self) -> Option<&WizardStep> {
        self.steps.get(self.selected_index)
    }

    /// The guided command the `c` key should copy: the command at
    /// `selected_command_index` of the currently selected step, if any.
    ///
    /// Returns `None` when the step list is empty, the selected step has no
    /// guided commands, or `selected_command_index` is out of range.
    /// Intended for use by the key handler that copies to the system clipboard.
    pub fn selected_guided_command(&self) -> Option<&GuidedCommand> {
        let step = self.steps.get(self.selected_index)?;
        step.guided_commands.get(self.selected_command_index)
    }

    /// Advance `selected_command_index` by 1, clamped to the last valid index.
    ///
    /// No-op when the selected step has 0 or 1 guided commands.
    pub fn select_next_command(&mut self) {
        let len = self
            .steps
            .get(self.selected_index)
            .map(|s| s.guided_commands.len())
            .unwrap_or(0);
        if len <= 1 {
            return;
        }
        if self.selected_command_index < len - 1 {
            self.selected_command_index += 1;
        }
    }

    /// Retreat `selected_command_index` by 1, saturating at 0.
    ///
    /// No-op when the selected step has 0 or 1 guided commands.
    pub fn select_prev_command(&mut self) {
        let len = self
            .steps
            .get(self.selected_index)
            .map(|s| s.guided_commands.len())
            .unwrap_or(0);
        if len <= 1 {
            return;
        }
        self.selected_command_index = self.selected_command_index.saturating_sub(1);
    }

    /// Whether a step is currently executing.
    ///
    /// Returns `true` only when `execution.status == Running`. Used by handlers
    /// to guard against concurrent step runs.
    pub fn is_step_running(&self) -> bool {
        self.execution.status == StepExecStatus::Running
    }

    /// Returns `true` when the last preflight report shows the Flutter SDK as live
    /// (at least one `FlutterSdk` component with `ComponentStatus::Ok`).
    ///
    /// Used as the handback predicate in `handle_preflight_completed` and
    /// `handle_hide`/`handle_escape`.  Returns `false` when no report has been
    /// applied yet (early-exit before the first preflight completes).
    ///
    /// Note: this reads the *report*, not `AppState::resolved_sdk`.  After a
    /// managed install the preflight re-run calls `find_flutter_sdk` itself and
    /// reflects the result in `report.components`, so this predicate is reliable
    /// once `apply_report` has processed the post-install report.
    pub fn flutter_now_live(&self) -> bool {
        let report = match self.report.as_ref() {
            Some(r) => r,
            None => return false,
        };
        report
            .components
            .iter()
            .any(|c| c.kind == ComponentKind::FlutterSdk && c.status == ComponentStatus::Ok)
    }

    /// Begin a run: set `Running`, clear prior progress/log/summary, record the
    /// step kind, clear any previous task handle, and bump the run sequence.
    ///
    /// Called **only** by [`handle_run_selected_step`] when a new step execution
    /// starts. Clears `install_task` so a new run never inherits a stale handle
    /// (F8), and bumps `run_seq` so late `WizardInstallTaskReady` messages from
    /// the previous run are rejected.
    ///
    /// `handle_step_started` does **not** call this method. Instead it calls
    /// [`reset_progress_display`][Self::reset_progress_display] (which only
    /// resets the display fields) when the step is already Running for the same
    /// kind — preserving the synchronously-stored `install_task` and `run_seq`.
    pub fn begin_step(&mut self, kind: WizardStepKind) {
        // Clear any prior handle so a new run never inherits a stale one (F8).
        let _ = self.install_task.take();
        // Bump the sequence counter so late `WizardInstallTaskReady` messages
        // from the previous run are rejected by `handle_install_task_ready`.
        self.run_seq = self.run_seq.wrapping_add(1);
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

    /// Reset only the progress display fields for an already-running step.
    ///
    /// Clears `phase_label`, `received`, `total`, `log_tail`, and
    /// `result_summary` without touching `install_task`, `run_seq`, `status`,
    /// or `kind`.
    ///
    /// Called by [`handle_step_started`] when the step is already `Running`
    /// for the correct kind (i.e. `handle_run_selected_step` already called
    /// `begin_step`). Gives the TUI a fresh display without clobbering the
    /// synchronously-stored cancellation token or the run-sequence counter.
    pub fn reset_progress_display(&mut self) {
        self.execution.phase_label = None;
        self.execution.received = 0;
        self.execution.total = None;
        self.execution.log_tail.clear();
        self.execution.result_summary = None;
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
    /// `status` should be `Succeeded`, `Failed`, or `Cancelled`; passing
    /// `Running` or `Idle` is a logic error but will not panic (the summary is
    /// still stored). After this call,
    /// [`is_step_running`][Self::is_step_running] returns `false`.
    pub fn finish_step(&mut self, status: StepExecStatus, summary: String) {
        self.execution.status = status;
        self.execution.result_summary = Some(summary);
        // Clear the task handle on any terminal transition so it never lingers.
        let _ = self.install_task.take();
    }

    /// Reset a running step back to `Idle` without recording a terminal status.
    ///
    /// Used by [`handle_cancel_step`] after the cancellation token has been
    /// signalled, so that the user can press `Enter` to retry immediately.
    ///
    /// Also clears `install_task`; the caller should have already called
    /// `task.cancel.cancel()` before this.
    pub fn reset_running_step_to_idle(&mut self) {
        self.execution.status = StepExecStatus::Idle;
        self.execution.result_summary = None;
        self.execution.phase_label = None;
        // install_task already taken by the caller, but clear defensively.
        let _ = self.install_task.take();
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
            .field("selected_command_index", &self.selected_command_index)
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
            .field(
                "install_task",
                &self.install_task.as_ref().map(|_| "<running>"),
            )
            .field("run_seq", &self.run_seq)
            .field("handback_done", &self.handback_done)
            .field("origin", &self.origin)
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
///
/// For Linux the correct install command is chosen from the pre-detected
/// `report.linux_package_manager` so that the displayed command matches the
/// user's actual package manager — not a hardcoded `apt` fallback.
fn jdk_guided_command(report: &ToolchainReport) -> GuidedCommand {
    let (command, note) = match report.platform {
        HostPlatform::Linux => {
            let pm = report
                .linux_package_manager
                .unwrap_or(LinuxPackageManager::Unknown);
            match pm {
                LinuxPackageManager::Apt => (
                    "sudo apt install openjdk-17-jdk",
                    Some("or: sudo pacman -S jdk17-openjdk"),
                ),
                LinuxPackageManager::Dnf => (
                    "sudo dnf install java-17-openjdk-devel",
                    Some("or: sudo apt install openjdk-17-jdk"),
                ),
                LinuxPackageManager::Yum => (
                    "sudo yum install java-17-openjdk-devel",
                    Some("or: sudo apt install openjdk-17-jdk"),
                ),
                LinuxPackageManager::Pacman => (
                    "sudo pacman -S jdk17-openjdk",
                    Some("or: sudo pacman -S jre17-openjdk (runtime only)"),
                ),
                LinuxPackageManager::Zypper => (
                    "sudo zypper install java-17-openjdk-devel",
                    Some("or: sudo apt install openjdk-17-jdk"),
                ),
                LinuxPackageManager::Unknown => ("Install JDK 17 from https://adoptium.net", None),
            }
        }
        HostPlatform::MacOs => ("brew install openjdk@17", None),
        HostPlatform::Windows => ("winget install --id EclipseAdoptium.Temurin.17.JDK", None),
        HostPlatform::Unknown => ("Install JDK 17 from https://adoptium.net", None),
    };
    GuidedCommand {
        label: "Install JDK 17".into(),
        command: command.into(),
        note: note.map(Into::into),
    }
}

/// Map a probe key (from `parse_missing_prereq_keys`) to the package name for
/// the given Linux package manager.
///
/// Returns `None` for keys that have no known mapping — callers should filter
/// those out rather than emitting an empty package name.
///
/// The mapping follows the table documented in the phase-6 task spec:
///
/// | key            | apt               | dnf/yum          | pacman   | zypper           |
/// |----------------|-------------------|------------------|----------|------------------|
/// | git            | git               | git              | git      | git              |
/// | zip            | zip               | zip              | zip      | zip              |
/// | curl           | curl              | curl             | curl     | curl             |
/// | unzip          | unzip             | unzip            | unzip    | unzip            |
/// | xz             | xz-utils          | xz               | xz       | xz               |
/// | clang          | clang             | clang            | clang    | clang            |
/// | cmake          | cmake             | cmake            | cmake    | cmake            |
/// | ninja          | ninja-build       | ninja-build      | ninja    | ninja            |
/// | pkg-config     | pkg-config        | pkgconf          | pkgconf  | pkg-config       |
/// | libgtk-3-dev   | libgtk-3-dev      | gtk3-devel       | gtk3     | gtk3-devel       |
/// | libglu1-mesa   | libglu1-mesa      | mesa-libGLU      | glu      | Mesa-libGLU1     |
/// | libstdc++      | libstdc++-12-dev  | libstdc++-devel  | gcc      | libstdc++-devel  |
pub(crate) fn linux_package_name(key: &str, pm: LinuxPackageManager) -> Option<&'static str> {
    // Table: (key, apt, dnf, yum, pacman, zypper)
    // dnf and yum share the same package names.
    const TABLE: &[(&str, &str, &str, &str, &str)] = &[
        //          key             apt                  dnf/yum              pacman     zypper
        ("git", "git", "git", "git", "git"),
        ("zip", "zip", "zip", "zip", "zip"),
        ("curl", "curl", "curl", "curl", "curl"),
        ("unzip", "unzip", "unzip", "unzip", "unzip"),
        ("xz", "xz-utils", "xz", "xz", "xz"),
        ("clang", "clang", "clang", "clang", "clang"),
        ("cmake", "cmake", "cmake", "cmake", "cmake"),
        ("ninja", "ninja-build", "ninja-build", "ninja", "ninja"),
        (
            "pkg-config",
            "pkg-config",
            "pkgconf",
            "pkgconf",
            "pkg-config",
        ),
        (
            PREREQ_KEY_GTK_INTERNAL,
            "libgtk-3-dev",
            "gtk3-devel",
            "gtk3",
            "gtk3-devel",
        ),
        (
            PREREQ_KEY_GLU,
            "libglu1-mesa",
            "mesa-libGLU",
            "glu",
            "Mesa-libGLU1",
        ),
        (
            PREREQ_KEY_LIBSTDCPP,
            "libstdc++-12-dev",
            "libstdc++-devel",
            "gcc",
            "libstdc++-devel",
        ),
    ];

    for (row_key, apt, dnf_yum, pacman, zypper) in TABLE {
        if *row_key == key {
            return Some(match pm {
                LinuxPackageManager::Apt => apt,
                LinuxPackageManager::Dnf | LinuxPackageManager::Yum => dnf_yum,
                LinuxPackageManager::Pacman => pacman,
                LinuxPackageManager::Zypper => zypper,
                LinuxPackageManager::Unknown => return None,
            });
        }
    }
    None
}

/// The GTK dev-header key as it appears in the `missing:` detail on Linux.
///
/// The daemon encodes it as `"libgtk-3-dev"` (the apt package name).
const PREREQ_KEY_GTK_INTERNAL: &str = "libgtk-3-dev";

/// Return the install-command label and verb prefix for the given Linux package
/// manager.
///
/// The returned prefix is the full `sudo <pm> install …` prefix without
/// trailing space. Callers append `" <packages>"`.
fn linux_install_verb(pm: LinuxPackageManager) -> (&'static str, &'static str) {
    match pm {
        LinuxPackageManager::Apt => (
            "Install Linux prerequisites (apt)",
            "sudo apt-get install -y",
        ),
        LinuxPackageManager::Dnf => ("Install Linux prerequisites (dnf)", "sudo dnf install -y"),
        LinuxPackageManager::Yum => ("Install Linux prerequisites (yum)", "sudo yum install -y"),
        LinuxPackageManager::Pacman => (
            "Install Linux prerequisites (pacman)",
            "sudo pacman -S --needed",
        ),
        LinuxPackageManager::Zypper => ("Install Linux prerequisites (zypper)", "sudo zypper in"),
        LinuxPackageManager::Unknown => (
            "Install Linux prerequisites",
            "https://docs.flutter.dev/get-started/install/linux",
        ),
    }
}

/// Per-OS guided commands for the `Prerequisites` step.
///
/// Returns an empty `Vec` when all prerequisites/git checks are `Ok` — nothing
/// to show. Otherwise returns up to three `GuidedCommand`s depending on the
/// host platform and which items are missing.
///
/// - **Linux** — one combined command chosen by the pre-computed
///   `report.linux_package_manager`; uses the `note` field for an
///   alternative-manager hint (mirrors `jdk_guided_command`).
/// - **macOS** — one command per missing item reported by
///   [`parse_missing_prereq_keys`], ordered CLT → CocoaPods → Rosetta.
/// - **Windows** — `winget install Git.Git` when git is missing and
///   `report.winget_available` is `true`; otherwise a `note` pointing at the
///   git-for-Windows download page.
/// - **Unknown** — empty (no actionable commands).
///
/// This function is a **pure function of the report**: both the package-manager
/// detection (Linux) and winget availability (Windows) are pre-computed in the
/// async `run_preflight` task and carried on `ToolchainReport`, so no
/// synchronous `which::which` I/O occurs inside the TEA `update()` path.
///
/// All command strings live here (app display concern), consistent with
/// `jdk_guided_command` — the daemon stays detection-only.
fn prerequisites_guided_commands(
    report: &ToolchainReport,
    components: &[ComponentCheck],
) -> Vec<GuidedCommand> {
    // Early-out: nothing to do when all prerequisites/git are Ok.
    let all_ok = components
        .iter()
        .filter(|c| matches!(c.kind, ComponentKind::Prerequisites | ComponentKind::Git))
        .all(|c| c.status == ComponentStatus::Ok);

    if all_ok && !components.is_empty() {
        return Vec::new();
    }

    // If there are no prerequisite or git components at all, nothing to show.
    let has_prereq_or_git = components
        .iter()
        .any(|c| matches!(c.kind, ComponentKind::Prerequisites | ComponentKind::Git));
    if !has_prereq_or_git {
        return Vec::new();
    }

    match report.platform {
        HostPlatform::Linux => {
            // Use the package manager pre-computed by run_preflight (no which:: I/O here).
            let pm = report
                .linux_package_manager
                .unwrap_or(LinuxPackageManager::Unknown);

            // Unknown manager: fall back to the Flutter docs URL.
            if pm == LinuxPackageManager::Unknown {
                return vec![GuidedCommand {
                    label: "Install Linux prerequisites".into(),
                    command: "https://docs.flutter.dev/get-started/install/linux".into(),
                    note: None,
                }];
            }

            // Extract only the keys that are actually missing from the
            // Prerequisites component detail.  This mirrors the macOS/Windows
            // arms and avoids listing already-installed packages.
            let detail = components
                .iter()
                .find(|c| c.kind == ComponentKind::Prerequisites)
                .map(|c| c.detail.as_str())
                .unwrap_or("");
            let missing_keys = parse_missing_prereq_keys(detail);

            // If no missing keys are listed (all-Ok detail) return empty.
            if missing_keys.is_empty() {
                return Vec::new();
            }

            // Map each missing probe-key to the distro package name for this pm.
            let packages: Vec<&str> = missing_keys
                .iter()
                .filter_map(|key| linux_package_name(key, pm))
                .collect();

            if packages.is_empty() {
                return Vec::new();
            }

            let (label, install_prefix) = linux_install_verb(pm);
            let command = format!("{} {}", install_prefix, packages.join(" "));

            vec![GuidedCommand {
                label: label.into(),
                command,
                note: Some("Package names are best-effort; consult your distro docs if a package is not found.".into()),
            }]
        }

        HostPlatform::MacOs => {
            // Find the Prerequisites component detail to extract missing keys.
            // TODO(phase-4-followup n3): the stringly-typed detail → parse_missing_prereq_keys
            // cross-crate contract could be replaced by a typed Vec<&'static str> / enum-set
            // field on ComponentCheck, eliminating the parse path entirely.
            let detail = components
                .iter()
                .find(|c| c.kind == ComponentKind::Prerequisites)
                .map(|c| c.detail.as_str())
                .unwrap_or("");
            let missing_keys = parse_missing_prereq_keys(detail);

            let mut cmds: Vec<GuidedCommand> = Vec::new();

            // Order: CLT → CocoaPods → Rosetta (most-likely-missing first).
            if missing_keys.contains(&PREREQ_KEY_XCODE_CLT) {
                cmds.push(GuidedCommand {
                    label: "Install Xcode Command Line Tools".into(),
                    command: "xcode-select --install".into(),
                    note: Some("Opens a GUI dialog to install CLT.".into()),
                });
            }
            if missing_keys.contains(&PREREQ_KEY_COCOAPODS) {
                cmds.push(GuidedCommand {
                    label: "Install CocoaPods".into(),
                    command: "brew install cocoapods".into(),
                    note: Some("or: sudo gem install cocoapods".into()),
                });
            }
            if missing_keys.contains(&PREREQ_KEY_ROSETTA) {
                cmds.push(GuidedCommand {
                    label: "Install Rosetta 2".into(),
                    command: "sudo softwareupdate --install-rosetta --agree-to-license".into(),
                    note: None,
                });
            }

            cmds
        }

        HostPlatform::Windows => {
            // Check if git is among the missing keys.
            // TODO(phase-4-followup n3): the stringly-typed detail → parse_missing_prereq_keys
            // cross-crate contract could be replaced by a typed Vec<&'static str> / enum-set
            // field on ComponentCheck, eliminating the parse path entirely.
            let detail = components
                .iter()
                .find(|c| c.kind == ComponentKind::Prerequisites)
                .map(|c| c.detail.as_str())
                .unwrap_or("");
            let missing_keys = parse_missing_prereq_keys(detail);

            if !missing_keys.contains(&PREREQ_KEY_GIT) {
                // Git is present — no guided command needed.
                return Vec::new();
            }

            // Git is missing — use pre-computed winget availability from preflight.
            // (No which::which call here — pure function of the report.)
            //
            // After installing, the user should press `r` to re-check. fdemon
            // refreshes its process PATH from the registry at the start of every
            // preflight, so the re-check will find the newly-installed git without
            // restarting fdemon. Their own already-open terminals still need a
            // new window to inherit the updated PATH.
            if report.winget_available {
                vec![GuidedCommand {
                    label: "Install Git for Windows".into(),
                    command: "winget install Git.Git".into(),
                    note: Some(
                        "After installing, press r to re-check. \
                         Your own already-open terminals still need a new window."
                            .into(),
                    ),
                }]
            } else {
                vec![GuidedCommand {
                    label: "Install Git for Windows".into(),
                    command: "https://git-scm.com/downloads/win".into(),
                    note: Some(
                        "Download and run the installer, then press r to re-check. \
                         Your own already-open terminals still need a new window."
                            .into(),
                    ),
                }]
            }
        }

        HostPlatform::Unknown => Vec::new(),
    }
}

/// Return `true` when a JDK needs user attention: the component list has no
/// `Jdk` entry at all, or the entry is not `Ok`.
///
/// This is the **single source of truth** for the JDK-actionable predicate,
/// used by both:
/// - `build_steps()` — to decide whether to populate guided commands for the
///   `AndroidTools` step.
/// - `actions.rs` `handle_run_selected_step()` — to gate the `AndroidTools`
///   executor (sdkmanager requires a JDK 17).
///
/// Both callers now agree: if no `Jdk` entry exists, a guided command is shown
/// **and** the executor is blocked.
pub(crate) fn is_jdk_actionable(components: &[ComponentCheck]) -> bool {
    match components.iter().find(|c| c.kind == ComponentKind::Jdk) {
        None => true, // No Jdk entry → assume missing
        Some(c) => c.status != ComponentStatus::Ok,
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

    // Derive guided commands for the AndroidTools step using the shared
    // `is_jdk_actionable` helper. This ensures the gate in `actions.rs` and the
    // guided-command population here agree exactly:
    // - No Jdk entry  → actionable (show command + block executor)
    // - Jdk non-Ok    → actionable (show command + block executor)
    // - Jdk Ok        → not actionable (no command, executor allowed)
    let android_guided: Vec<GuidedCommand> = if is_jdk_actionable(&android_tools) {
        vec![jdk_guided_command(report)]
    } else {
        Vec::new()
    };

    // Derive guided commands for the Prerequisites step. Returns [] when all
    // prerequisites are Ok, otherwise returns per-OS install commands derived
    // from the pre-computed package manager (Linux) or missing keys (macOS/Windows).
    // Pure function of the report — no I/O in the TEA update() path.
    let prereq_guided = prerequisites_guided_commands(report, &prerequisites);

    vec![
        WizardStep {
            kind: WizardStepKind::Prerequisites,
            title: "Prerequisites".to_string(),
            status: prerequisites_status,
            components: prerequisites,
            guided_commands: prereq_guided,
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
    /// Uses Linux platform with no package manager (Unknown) and winget=false.
    fn make_report(components: Vec<ComponentCheck>) -> ToolchainReport {
        ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components,
            doctor: None,
            linux_package_manager: Some(LinuxPackageManager::Unknown),
            winget_available: false,
        }
    }

    /// Build a `ToolchainReport` for testing with the given platform and components.
    fn make_report_for_platform(
        platform: HostPlatform,
        components: Vec<ComponentCheck>,
    ) -> ToolchainReport {
        let linux_package_manager = if matches!(platform, HostPlatform::Linux) {
            Some(LinuxPackageManager::Unknown)
        } else {
            None
        };
        ToolchainReport {
            platform,
            shell: HostShell::Bash,
            components,
            doctor: None,
            linux_package_manager,
            winget_available: false,
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
        let s = InstallWizardState::opening(WizardOrigin::UserInvoked);
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
        let mut state = InstallWizardState::opening(WizardOrigin::UserInvoked);
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

    /// F-PR53-12: `apply_report` must reset a stale `execution` back to
    /// `Idle` so the static component list renders over a previous
    /// Failed/Cancelled/Succeeded view.
    #[test]
    fn test_apply_report_resets_execution() {
        let mut state = InstallWizardState::opening(WizardOrigin::UserInvoked);

        // Simulate a completed (failed) step.
        state.begin_step(WizardStepKind::FlutterSdk);
        state.finish_step(StepExecStatus::Failed, "network timeout".to_string());
        assert_eq!(state.execution.status, StepExecStatus::Failed);
        assert_eq!(state.execution.kind, Some(WizardStepKind::FlutterSdk));

        // Apply a fresh report (e.g. user fixed the issue and re-checked).
        let report = make_report(vec![make_check(
            ComponentKind::FlutterSdk,
            ComponentStatus::Ok,
        )]);
        state.apply_report(report);

        // execution must be back to default (Idle, kind = None).
        assert_eq!(
            state.execution.status,
            StepExecStatus::Idle,
            "apply_report must reset execution.status to Idle"
        );
        assert_eq!(
            state.execution.kind, None,
            "apply_report must reset execution.kind to None"
        );
        assert!(
            state.execution.result_summary.is_none(),
            "apply_report must clear execution.result_summary"
        );
    }

    /// F-PR53-12 (Cancelled variant): `apply_report` must also clear a
    /// stale `Cancelled` execution so the component list renders.
    #[test]
    fn test_apply_report_resets_cancelled_execution() {
        let mut state = InstallWizardState::opening(WizardOrigin::UserInvoked);

        state.begin_step(WizardStepKind::AndroidTools);
        state.finish_step(
            StepExecStatus::Cancelled,
            "Cancelled: user pressed Esc".to_string(),
        );
        assert_eq!(state.execution.status, StepExecStatus::Cancelled);

        let report = make_report(vec![make_check(
            ComponentKind::AndroidCmdlineTools,
            ComponentStatus::Ok,
        )]);
        state.apply_report(report);

        assert_eq!(
            state.execution.status,
            StepExecStatus::Idle,
            "apply_report must reset Cancelled execution to Idle"
        );
        assert_eq!(state.execution.kind, None);
    }

    /// F-PR53-12 (Succeeded variant): `apply_report` after an auto-triggered
    /// re-check (e.g. AndroidTools/PathConfig success) must clear the stale
    /// `Succeeded` execution.
    #[test]
    fn test_apply_report_resets_succeeded_execution() {
        let mut state = InstallWizardState::opening(WizardOrigin::UserInvoked);

        state.begin_step(WizardStepKind::AndroidTools);
        state.finish_step(
            StepExecStatus::Succeeded,
            "Android tools installed".to_string(),
        );
        assert_eq!(state.execution.status, StepExecStatus::Succeeded);

        let report = make_report(vec![make_check(
            ComponentKind::AndroidCmdlineTools,
            ComponentStatus::Ok,
        )]);
        state.apply_report(report);

        assert_eq!(
            state.execution.status,
            StepExecStatus::Idle,
            "apply_report must reset Succeeded execution to Idle"
        );
        assert_eq!(state.execution.kind, None);
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
        let linux_package_manager = if matches!(platform, HostPlatform::Linux) {
            Some(LinuxPackageManager::Unknown)
        } else {
            None
        };
        ToolchainReport {
            platform,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::Jdk,
                status,
                detail: String::new(),
            }],
            doctor: None,
            linux_package_manager,
            winget_available: false,
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
    fn test_jdk_command_linux_apt_uses_apt() {
        // Verify the Apt arm specifically — inject Apt as the package manager.
        let report = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::Jdk,
                status: ComponentStatus::Missing,
                detail: String::new(),
            }],
            doctor: None,
            linux_package_manager: Some(LinuxPackageManager::Apt),
            winget_available: false,
        };
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        let cmd = &android.guided_commands[0];
        assert_eq!(cmd.label, "Install JDK 17");
        assert!(
            cmd.command.contains("apt"),
            "Apt manager must use apt; got: {}",
            cmd.command
        );
        assert!(
            cmd.command.contains("openjdk-17-jdk"),
            "Apt arm must install openjdk-17-jdk; got: {}",
            cmd.command
        );
        assert!(
            cmd.note.is_some(),
            "Apt arm should have an alternative note"
        );
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

    /// The `report_with_jdk` fixture only contains a Jdk component — no
    /// Prerequisites/Git entries — so `prerequisites_guided_commands`
    /// short-circuits to an empty vec.  This test asserts the invariant
    /// *for that fixture* (JDK missing, no prereq components).
    #[test]
    fn test_non_android_non_prereq_steps_have_no_guided_commands_when_prereqs_absent() {
        let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Linux);
        let steps = build_steps(&report);
        for step in &steps {
            if step.kind != WizardStepKind::AndroidTools {
                assert!(
                    step.guided_commands.is_empty(),
                    "Step {:?} should have no guided commands (prereqs absent fixture)",
                    step.kind
                );
            }
        }
    }

    /// `PathConfig`, `FlutterSdk`, and `Doctor` must never carry guided
    /// commands regardless of which other components are present.
    #[test]
    fn test_path_config_flutter_sdk_doctor_never_have_guided_commands() {
        // Build a report that exercises all component kinds (all Ok so that
        // prerequisites_guided_commands and jdk guidance are both silent).
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
        for kind in [
            WizardStepKind::PathConfig,
            WizardStepKind::FlutterSdk,
            WizardStepKind::Doctor,
        ] {
            let step = steps
                .iter()
                .find(|s| s.kind == kind)
                .unwrap_or_else(|| panic!("{kind:?} step must be present in build_steps output"));
            assert!(
                step.guided_commands.is_empty(),
                "{kind:?} must never have guided commands"
            );
        }
    }

    // --- is_jdk_actionable edge-case tests (m2) ---

    #[test]
    fn test_is_jdk_actionable_no_jdk_entry_returns_true() {
        // When no Jdk component is present in the list, the helper must treat
        // the JDK as actionable (missing → show guided command, block executor).
        let components: Vec<ComponentCheck> = vec![
            make_check(ComponentKind::AndroidCmdlineTools, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidPlatformTools, ComponentStatus::Ok),
        ];
        assert!(
            is_jdk_actionable(&components),
            "no Jdk entry → is_jdk_actionable must return true"
        );
    }

    #[test]
    fn test_is_jdk_actionable_empty_list_returns_true() {
        // Empty component list → no Jdk entry → actionable.
        let components: Vec<ComponentCheck> = vec![];
        assert!(
            is_jdk_actionable(&components),
            "empty component list → is_jdk_actionable must return true"
        );
    }

    #[test]
    fn test_is_jdk_actionable_jdk_ok_returns_false() {
        let components = vec![make_check(ComponentKind::Jdk, ComponentStatus::Ok)];
        assert!(
            !is_jdk_actionable(&components),
            "Jdk Ok → is_jdk_actionable must return false"
        );
    }

    #[test]
    fn test_is_jdk_actionable_jdk_missing_returns_true() {
        let components = vec![make_check(ComponentKind::Jdk, ComponentStatus::Missing)];
        assert!(is_jdk_actionable(&components));
    }

    #[test]
    fn test_is_jdk_actionable_jdk_partial_returns_true() {
        let components = vec![make_check(ComponentKind::Jdk, ComponentStatus::Partial)];
        assert!(is_jdk_actionable(&components));
    }

    #[test]
    fn test_is_jdk_actionable_jdk_error_returns_true() {
        let components = vec![make_check(ComponentKind::Jdk, ComponentStatus::Error)];
        assert!(is_jdk_actionable(&components));
    }

    #[test]
    fn test_build_steps_no_jdk_entry_shows_guided_command() {
        // A report where android_tools has NO Jdk entry: the guided command must
        // still appear because is_jdk_actionable returns true for an absent entry.
        // This ensures the gate ("see the command below") and the rendered command agree.
        let report = make_report(vec![make_check(
            ComponentKind::AndroidCmdlineTools,
            ComponentStatus::Missing,
        )]);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .expect("AndroidTools step must exist");
        assert_eq!(
            android.guided_commands.len(),
            1,
            "AndroidTools with no Jdk entry must show a guided command (m2 fix)"
        );
        assert!(
            android.guided_commands[0].command.contains("17"),
            "guided command must reference JDK 17"
        );
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

    // --- selected_command_index tests ---

    #[test]
    fn test_selected_command_index_defaults_to_zero() {
        let state = InstallWizardState::default();
        assert_eq!(state.selected_command_index, 0);
    }

    #[test]
    fn test_apply_report_resets_selected_command_index() {
        let mut state = InstallWizardState {
            selected_command_index: 2,
            ..InstallWizardState::default()
        };
        let report = make_report(vec![make_check(
            ComponentKind::FlutterSdk,
            ComponentStatus::Ok,
        )]);
        state.apply_report(report);
        assert_eq!(
            state.selected_command_index, 0,
            "apply_report must reset selected_command_index to 0"
        );
    }

    /// Build a state with a macOS Prerequisites step that has 3 guided commands
    /// (CLT + CocoaPods + Rosetta all missing).
    fn state_with_three_prereq_commands() -> InstallWizardState {
        use fdemon_daemon::toolchain::{
            HostShell, PREREQ_KEY_COCOAPODS, PREREQ_KEY_ROSETTA, PREREQ_KEY_XCODE_CLT,
        };
        let detail = format!(
            "missing: {}, {}, {}",
            PREREQ_KEY_XCODE_CLT, PREREQ_KEY_COCOAPODS, PREREQ_KEY_ROSETTA
        );
        let report = ToolchainReport {
            platform: HostPlatform::MacOs,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::Prerequisites,
                status: ComponentStatus::Missing,
                detail,
            }],
            doctor: None,
            linux_package_manager: None,
            winget_available: false,
        };
        let mut state = InstallWizardState::default();
        state.apply_report(report);
        // Select the Prerequisites step (index 0), which has 3 commands.
        state.selected_index = 0;
        state
    }

    #[test]
    fn test_select_next_command_advances_index() {
        let mut state = state_with_three_prereq_commands();
        assert_eq!(state.selected_command_index, 0);
        state.select_next_command();
        assert_eq!(state.selected_command_index, 1);
        state.select_next_command();
        assert_eq!(state.selected_command_index, 2);
    }

    #[test]
    fn test_select_next_command_clamps_at_last() {
        let mut state = state_with_three_prereq_commands();
        state.selected_command_index = 2;
        state.select_next_command();
        assert_eq!(state.selected_command_index, 2, "must clamp at last index");
    }

    #[test]
    fn test_select_prev_command_retreats_index() {
        let mut state = state_with_three_prereq_commands();
        state.selected_command_index = 2;
        state.select_prev_command();
        assert_eq!(state.selected_command_index, 1);
        state.select_prev_command();
        assert_eq!(state.selected_command_index, 0);
    }

    #[test]
    fn test_select_prev_command_saturates_at_zero() {
        let mut state = state_with_three_prereq_commands();
        state.selected_command_index = 0;
        state.select_prev_command();
        assert_eq!(state.selected_command_index, 0, "must saturate at 0");
    }

    #[test]
    fn test_select_next_noop_for_single_command_step() {
        let mut state = InstallWizardState::default();
        let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Linux);
        state.apply_report(report);
        // AndroidTools (index 1) has exactly 1 guided command.
        state.selected_index = 1;
        state.selected_command_index = 0;
        state.select_next_command();
        assert_eq!(
            state.selected_command_index, 0,
            "next must be no-op for single-command step"
        );
    }

    #[test]
    fn test_select_prev_noop_for_single_command_step() {
        let mut state = InstallWizardState::default();
        let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Linux);
        state.apply_report(report);
        // AndroidTools (index 1) has exactly 1 guided command.
        state.selected_index = 1;
        state.selected_command_index = 0;
        state.select_prev_command();
        assert_eq!(
            state.selected_command_index, 0,
            "prev must be no-op for single-command step"
        );
    }

    #[test]
    fn test_select_next_noop_for_no_command_step() {
        let mut state = InstallWizardState::default();
        let report = make_report(vec![make_check(
            ComponentKind::FlutterSdk,
            ComponentStatus::Ok,
        )]);
        state.apply_report(report);
        // PathConfig (index 2) has 0 guided commands.
        state.selected_index = 2;
        state.selected_command_index = 0;
        state.select_next_command();
        assert_eq!(
            state.selected_command_index, 0,
            "next must be no-op for zero-command step"
        );
    }

    #[test]
    fn test_select_prev_noop_for_no_command_step() {
        let mut state = InstallWizardState::default();
        let report = make_report(vec![make_check(
            ComponentKind::FlutterSdk,
            ComponentStatus::Ok,
        )]);
        state.apply_report(report);
        // PathConfig (index 2) has 0 guided commands.
        state.selected_index = 2;
        state.selected_command_index = 0;
        state.select_prev_command();
        assert_eq!(
            state.selected_command_index, 0,
            "prev must be no-op for zero-command step"
        );
    }

    #[test]
    fn test_selected_guided_command_uses_index() {
        let mut state = state_with_three_prereq_commands();
        // Index 0 → CLT
        assert_eq!(
            state.selected_guided_command().map(|c| c.label.as_str()),
            Some("Install Xcode Command Line Tools")
        );
        // Index 1 → CocoaPods
        state.selected_command_index = 1;
        assert_eq!(
            state.selected_guided_command().map(|c| c.label.as_str()),
            Some("Install CocoaPods")
        );
        // Index 2 → Rosetta
        state.selected_command_index = 2;
        assert_eq!(
            state.selected_guided_command().map(|c| c.label.as_str()),
            Some("Install Rosetta 2")
        );
    }

    #[test]
    fn test_selected_guided_command_returns_none_for_out_of_range_index() {
        let mut state = InstallWizardState::default();
        let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Linux);
        state.apply_report(report);
        // AndroidTools (index 1) has 1 command.
        state.selected_index = 1;
        // Manually set out-of-range index (defensive clamping).
        state.selected_command_index = 99;
        assert!(
            state.selected_guided_command().is_none(),
            "out-of-range index must return None"
        );
    }

    // --- prerequisites_guided_commands tests ---

    fn make_prereq_check(status: ComponentStatus) -> ComponentCheck {
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status,
            detail: String::new(),
        }
    }

    fn make_prereq_check_with_detail(status: ComponentStatus, detail: &str) -> ComponentCheck {
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status,
            detail: detail.to_string(),
        }
    }

    fn make_git_check(status: ComponentStatus) -> ComponentCheck {
        ComponentCheck {
            kind: ComponentKind::Git,
            status,
            detail: String::new(),
        }
    }

    #[test]
    fn test_prereq_guided_empty_when_all_ok() {
        // When both Prerequisites and Git are Ok, no commands needed.
        let components = vec![
            make_prereq_check(ComponentStatus::Ok),
            make_git_check(ComponentStatus::Ok),
        ];
        let report = make_report_for_platform(HostPlatform::Linux, components.clone());
        let cmds = prerequisites_guided_commands(&report, &components);
        assert!(
            cmds.is_empty(),
            "must return empty when all prereqs are Ok; got: {cmds:?}"
        );
    }

    #[test]
    fn test_prereq_guided_empty_when_no_prereq_components() {
        // No Prerequisites/Git components at all → nothing to show.
        let components: Vec<ComponentCheck> = vec![];
        let report = make_report_for_platform(HostPlatform::Linux, components.clone());
        let cmds = prerequisites_guided_commands(&report, &components);
        assert!(
            cmds.is_empty(),
            "must return empty when no prereq components"
        );
    }

    #[test]
    fn test_prereq_guided_linux_apt_returns_one_command() {
        // Simulate: prerequisites partial (curl + git missing) on Linux.
        // Use the live-detected package manager to match the host environment.
        let pm = fdemon_daemon::toolchain::detect_linux_package_manager();
        // Provide a proper missing-key detail so the filter returns a non-empty list.
        let detail = "missing: curl, git";
        let components = vec![make_prereq_check_with_detail(
            ComponentStatus::Partial,
            detail,
        )];
        let report = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: components.clone(),
            doctor: None,
            linux_package_manager: Some(pm),
            winget_available: false,
        };
        let cmds = prerequisites_guided_commands(&report, &components);
        // Unknown PM returns the docs URL (1 command), known PMs return install commands.
        assert_eq!(cmds.len(), 1, "Linux must return exactly one command");
        assert!(
            !cmds[0].command.is_empty(),
            "command must not be empty on Linux"
        );
        // The command must mention the missing packages or Flutter docs URL.
        assert!(
            cmds[0].command.contains("curl")
                || cmds[0].command.contains("flutter")
                || cmds[0].command.contains("https://"),
            "Linux command must reference install packages or Flutter docs URL; got: {}",
            cmds[0].command
        );
    }

    #[test]
    fn test_prereq_guided_linux_apt_command_content() {
        // Test the command string by providing known-missing keys.
        // The report carries the pre-computed PM — no live detection inside the function.
        let pm = fdemon_daemon::toolchain::detect_linux_package_manager();
        // Detail with libgtk-3-dev missing so we can assert its package name.
        let detail = "missing: curl, libgtk-3-dev";
        let components = vec![make_prereq_check_with_detail(
            ComponentStatus::Partial,
            detail,
        )];
        let report = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: components.clone(),
            doctor: None,
            linux_package_manager: Some(pm),
            winget_available: false,
        };
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(cmds.len(), 1);

        // Verify the command matches the detected package manager.
        match pm {
            LinuxPackageManager::Apt => {
                assert!(
                    cmds[0].command.contains("apt-get"),
                    "apt system should use apt-get; got: {}",
                    cmds[0].command
                );
                assert!(
                    cmds[0].command.contains("libgtk-3-dev"),
                    "apt command must include libgtk-3-dev; got: {}",
                    cmds[0].command
                );
                assert!(
                    cmds[0].note.is_some(),
                    "apt command must have an alternative note"
                );
            }
            LinuxPackageManager::Dnf => {
                assert!(
                    cmds[0].command.contains("dnf"),
                    "dnf system should use dnf; got: {}",
                    cmds[0].command
                );
                let note = cmds[0].note.as_deref().unwrap_or("");
                assert!(
                    note.contains("best-effort"),
                    "dnf note must contain best-effort caveat; got: {note}"
                );
            }
            LinuxPackageManager::Yum => {
                assert!(
                    cmds[0].command.contains("yum"),
                    "yum system should use yum (not dnf); got: {}",
                    cmds[0].command
                );
                assert!(
                    !cmds[0].command.contains("dnf"),
                    "yum arm must not invoke dnf; got: {}",
                    cmds[0].command
                );
                assert!(
                    cmds[0].note.is_some(),
                    "yum command must have a caveat note"
                );
            }
            LinuxPackageManager::Pacman => {
                assert!(cmds[0].command.contains("pacman"));
                assert!(cmds[0].command.contains("--needed"));
                let note = cmds[0].note.as_deref().unwrap_or("");
                assert!(
                    note.contains("best-effort"),
                    "pacman note must contain best-effort caveat; got: {note}"
                );
            }
            LinuxPackageManager::Zypper => {
                assert!(cmds[0].command.contains("zypper"));
                let note = cmds[0].note.as_deref().unwrap_or("");
                assert!(
                    note.contains("best-effort"),
                    "zypper note must contain best-effort caveat; got: {note}"
                );
            }
            LinuxPackageManager::Unknown => {
                assert!(
                    cmds[0].command.contains("https://"),
                    "Unknown PM must use docs URL"
                );
                assert!(cmds[0].note.is_none());
            }
        }
    }

    /// Verify specific package manager arms by injecting a known PM into the report.
    /// This tests the pure dispatch logic without depending on the host environment.
    #[test]
    fn test_prereq_guided_linux_pm_dispatch_pure() {
        // Use a detail with known missing keys so the filter is non-empty.
        // libgtk-3-dev and curl are in the mapping table for all managers.
        let detail = "missing: curl, libgtk-3-dev";
        let components = vec![make_prereq_check_with_detail(
            ComponentStatus::Partial,
            detail,
        )];

        // Test Apt arm
        let report_apt = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: components.clone(),
            doctor: None,
            linux_package_manager: Some(LinuxPackageManager::Apt),
            winget_available: false,
        };
        let cmds = prerequisites_guided_commands(&report_apt, &components);
        assert_eq!(cmds.len(), 1);
        assert!(
            cmds[0].command.contains("apt-get"),
            "Apt arm must use apt-get; got: {}",
            cmds[0].command
        );
        assert!(
            cmds[0].command.contains("libgtk-3-dev"),
            "Apt arm must include libgtk-3-dev; got: {}",
            cmds[0].command
        );
        assert!(
            cmds[0].note.is_some(),
            "Apt arm must have an alternative note"
        );

        // Test Dnf arm
        let report_dnf = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: components.clone(),
            doctor: None,
            linux_package_manager: Some(LinuxPackageManager::Dnf),
            winget_available: false,
        };
        let cmds = prerequisites_guided_commands(&report_dnf, &components);
        assert_eq!(cmds.len(), 1);
        assert!(
            cmds[0].command.contains("dnf"),
            "Dnf arm must use dnf; got: {}",
            cmds[0].command
        );
        assert!(
            cmds[0].command.contains("gtk3-devel"),
            "Dnf arm must map libgtk-3-dev → gtk3-devel; got: {}",
            cmds[0].command
        );
        {
            let note = cmds[0].note.as_deref().unwrap_or("");
            assert!(
                note.contains("best-effort"),
                "Dnf arm note must contain best-effort caveat; got: {note}"
            );
        }

        // Test Yum arm
        let report_yum = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: components.clone(),
            doctor: None,
            linux_package_manager: Some(LinuxPackageManager::Yum),
            winget_available: false,
        };
        let cmds = prerequisites_guided_commands(&report_yum, &components);
        assert_eq!(cmds.len(), 1);
        assert!(
            cmds[0].command.contains("yum"),
            "Yum arm must use yum; got: {}",
            cmds[0].command
        );
        assert!(
            !cmds[0].command.contains("dnf"),
            "Yum arm must not call dnf; got: {}",
            cmds[0].command
        );
        assert!(cmds[0].note.is_some(), "Yum arm must have a caveat note");

        // Test Pacman arm
        let report_pacman = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: components.clone(),
            doctor: None,
            linux_package_manager: Some(LinuxPackageManager::Pacman),
            winget_available: false,
        };
        let cmds = prerequisites_guided_commands(&report_pacman, &components);
        assert_eq!(cmds.len(), 1);
        assert!(
            cmds[0].command.contains("pacman"),
            "Pacman arm must use pacman; got: {}",
            cmds[0].command
        );
        assert!(
            cmds[0].command.contains("--needed"),
            "Pacman arm must use --needed; got: {}",
            cmds[0].command
        );
        assert!(
            cmds[0].command.contains("gtk3"),
            "Pacman arm must map libgtk-3-dev → gtk3; got: {}",
            cmds[0].command
        );
        {
            let note = cmds[0].note.as_deref().unwrap_or("");
            assert!(
                note.contains("best-effort"),
                "Pacman arm note must contain best-effort caveat; got: {note}"
            );
        }

        // Test Zypper arm
        let report_zypper = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: components.clone(),
            doctor: None,
            linux_package_manager: Some(LinuxPackageManager::Zypper),
            winget_available: false,
        };
        let cmds = prerequisites_guided_commands(&report_zypper, &components);
        assert_eq!(cmds.len(), 1);
        assert!(
            cmds[0].command.contains("zypper"),
            "Zypper arm must use zypper; got: {}",
            cmds[0].command
        );
        assert!(
            cmds[0].command.contains("gtk3-devel"),
            "Zypper arm must map libgtk-3-dev → gtk3-devel; got: {}",
            cmds[0].command
        );
        {
            let note = cmds[0].note.as_deref().unwrap_or("");
            assert!(
                note.contains("best-effort"),
                "Zypper arm note must contain best-effort caveat; got: {note}"
            );
        }

        // Test Unknown arm
        let report_unknown = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: components.clone(),
            doctor: None,
            linux_package_manager: Some(LinuxPackageManager::Unknown),
            winget_available: false,
        };
        let cmds = prerequisites_guided_commands(&report_unknown, &components);
        assert_eq!(cmds.len(), 1);
        assert!(
            cmds[0].command.contains("https://"),
            "Unknown PM arm must use docs URL; got: {}",
            cmds[0].command
        );
        assert!(cmds[0].note.is_none(), "Unknown PM arm must have no note");
    }

    /// Verify that winget_available=true from the report selects the winget command.
    #[test]
    fn test_prereq_guided_windows_winget_available_uses_winget_command() {
        use fdemon_daemon::toolchain::PREREQ_KEY_GIT;
        let detail = format!("missing: {}", PREREQ_KEY_GIT);
        let components = vec![make_prereq_check_with_detail(
            ComponentStatus::Missing,
            &detail,
        )];
        let report = ToolchainReport {
            platform: HostPlatform::Windows,
            shell: HostShell::Bash,
            components: components.clone(),
            doctor: None,
            linux_package_manager: None,
            winget_available: true, // pre-computed: winget IS available
        };
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(cmds.len(), 1);
        assert_eq!(
            cmds[0].command, "winget install Git.Git",
            "must use winget when available"
        );
        // The note now instructs users to press `r` to re-check (works in-process
        // on Windows after the registry PATH refresh) and notes that their own
        // already-open terminals still need a new window.
        assert!(
            cmds[0].note.is_some(),
            "winget arm must include a note about pressing r and new terminal windows"
        );
        let note = cmds[0].note.as_deref().unwrap();
        assert!(
            note.contains('r') || note.contains("re-check"),
            "note must mention re-check; got: {note}"
        );
    }

    /// Verify that winget_available=false from the report falls back to the URL.
    #[test]
    fn test_prereq_guided_windows_winget_unavailable_uses_url_fallback() {
        use fdemon_daemon::toolchain::PREREQ_KEY_GIT;
        let detail = format!("missing: {}", PREREQ_KEY_GIT);
        let components = vec![make_prereq_check_with_detail(
            ComponentStatus::Missing,
            &detail,
        )];
        let report = ToolchainReport {
            platform: HostPlatform::Windows,
            shell: HostShell::Bash,
            components: components.clone(),
            doctor: None,
            linux_package_manager: None,
            winget_available: false, // pre-computed: winget NOT available
        };
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(cmds.len(), 1);
        assert!(
            cmds[0].command.contains("git-scm.com"),
            "must use URL when winget unavailable"
        );
        assert!(cmds[0].note.is_some(), "URL fallback must have a note");
    }

    #[test]
    fn test_prereq_guided_macos_clt_missing() {
        use fdemon_daemon::toolchain::PREREQ_KEY_XCODE_CLT;
        let detail = format!("missing: {}", PREREQ_KEY_XCODE_CLT);
        let components = vec![make_prereq_check_with_detail(
            ComponentStatus::Missing,
            &detail,
        )];
        let report = make_report_for_platform(HostPlatform::MacOs, components.clone());
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(cmds.len(), 1, "one command for CLT only");
        assert!(
            cmds[0].command.contains("xcode-select"),
            "CLT command must use xcode-select; got: {}",
            cmds[0].command
        );
        // CocoaPods must NOT be in the list.
        assert!(
            !cmds.iter().any(|c| c.command.contains("cocoapods")),
            "cocoapods must not appear when only CLT is missing"
        );
    }

    #[test]
    fn test_prereq_guided_macos_cocoapods_missing() {
        use fdemon_daemon::toolchain::PREREQ_KEY_COCOAPODS;
        let detail = format!("missing: {}", PREREQ_KEY_COCOAPODS);
        let components = vec![make_prereq_check_with_detail(
            ComponentStatus::Missing,
            &detail,
        )];
        let report = make_report_for_platform(HostPlatform::MacOs, components.clone());
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(cmds.len(), 1);
        assert!(
            cmds[0].command.contains("cocoapods"),
            "CocoaPods command must use brew install cocoapods; got: {}",
            cmds[0].command
        );
        assert!(
            cmds[0].note.is_some(),
            "CocoaPods must have an alternative gem note"
        );
        assert!(
            cmds[0].note.as_deref().unwrap().contains("gem"),
            "alternative note must mention gem"
        );
    }

    #[test]
    fn test_prereq_guided_macos_rosetta_missing() {
        use fdemon_daemon::toolchain::PREREQ_KEY_ROSETTA;
        let detail = format!("missing: {}", PREREQ_KEY_ROSETTA);
        let components = vec![make_prereq_check_with_detail(
            ComponentStatus::Missing,
            &detail,
        )];
        let report = make_report_for_platform(HostPlatform::MacOs, components.clone());
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(cmds.len(), 1);
        assert!(
            cmds[0].command.contains("rosetta"),
            "Rosetta command must use softwareupdate; got: {}",
            cmds[0].command
        );
    }

    #[test]
    fn test_prereq_guided_macos_all_three_missing_ordered() {
        use fdemon_daemon::toolchain::{
            PREREQ_KEY_COCOAPODS, PREREQ_KEY_ROSETTA, PREREQ_KEY_XCODE_CLT,
        };
        let detail = format!(
            "missing: {}, {}, {}",
            PREREQ_KEY_XCODE_CLT, PREREQ_KEY_COCOAPODS, PREREQ_KEY_ROSETTA
        );
        let components = vec![make_prereq_check_with_detail(
            ComponentStatus::Missing,
            &detail,
        )];
        let report = make_report_for_platform(HostPlatform::MacOs, components.clone());
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(
            cmds.len(),
            3,
            "all three macOS missing items must be returned"
        );
        // Order: CLT → CocoaPods → Rosetta
        assert!(
            cmds[0].command.contains("xcode-select"),
            "first must be CLT"
        );
        assert!(
            cmds[1].command.contains("cocoapods"),
            "second must be CocoaPods"
        );
        assert!(cmds[2].command.contains("rosetta"), "third must be Rosetta");
    }

    #[test]
    fn test_prereq_guided_macos_ok_returns_empty() {
        let components = vec![make_prereq_check(ComponentStatus::Ok)];
        let report = make_report_for_platform(HostPlatform::MacOs, components.clone());
        let cmds = prerequisites_guided_commands(&report, &components);
        assert!(
            cmds.is_empty(),
            "macOS must return empty when prerequisites Ok"
        );
    }

    #[test]
    fn test_prereq_guided_windows_git_missing_no_winget() {
        // Simulate windows with git missing and winget NOT available (winget_available=false).
        use fdemon_daemon::toolchain::PREREQ_KEY_GIT;
        let detail = format!("missing: {}", PREREQ_KEY_GIT);
        let components = vec![make_prereq_check_with_detail(
            ComponentStatus::Missing,
            &detail,
        )];
        // Explicitly set winget_available=false to test the URL fallback.
        let report = ToolchainReport {
            platform: HostPlatform::Windows,
            shell: HostShell::Bash,
            components: components.clone(),
            doctor: None,
            linux_package_manager: None,
            winget_available: false,
        };
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(cmds.len(), 1, "one command for missing git");
        assert_eq!(cmds[0].label, "Install Git for Windows");
        assert!(
            cmds[0].command.contains("git-scm.com"),
            "must use URL fallback when winget_available=false; got: {}",
            cmds[0].command
        );
        assert!(cmds[0].note.is_some(), "URL fallback must have a note");
    }

    #[test]
    fn test_prereq_guided_windows_git_ok_returns_empty() {
        // Git present (Ok) on Windows → no guided command needed.
        let components = vec![make_prereq_check(ComponentStatus::Ok)];
        let report = make_report_for_platform(HostPlatform::Windows, components.clone());
        let cmds = prerequisites_guided_commands(&report, &components);
        assert!(
            cmds.is_empty(),
            "must return empty when windows prereqs are Ok"
        );
    }

    #[test]
    fn test_prereq_guided_unknown_platform_returns_empty() {
        let components = vec![make_prereq_check(ComponentStatus::Partial)];
        let report = make_report_for_platform(HostPlatform::Unknown, components.clone());
        let cmds = prerequisites_guided_commands(&report, &components);
        assert!(cmds.is_empty(), "Unknown platform must return empty");
    }

    #[test]
    fn test_build_steps_prereq_guided_wired_for_missing_prereqs() {
        // Verify that build_steps populates Prerequisites.guided_commands from
        // prerequisites_guided_commands (not always Vec::new()).
        use fdemon_daemon::toolchain::{HostShell, PREREQ_KEY_XCODE_CLT};
        let detail = format!("missing: {}", PREREQ_KEY_XCODE_CLT);
        let report = ToolchainReport {
            platform: HostPlatform::MacOs,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::Prerequisites,
                status: ComponentStatus::Missing,
                detail,
            }],
            doctor: None,
            linux_package_manager: None,
            winget_available: false,
        };
        let steps = build_steps(&report);
        let prereq = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::Prerequisites)
            .expect("Prerequisites step must exist");
        assert_eq!(
            prereq.guided_commands.len(),
            1,
            "Prerequisites step must have guided command when CLT missing"
        );
        assert!(
            prereq.guided_commands[0].command.contains("xcode-select"),
            "guided command must reference xcode-select; got: {}",
            prereq.guided_commands[0].command
        );
    }

    #[test]
    fn test_build_steps_prereq_guided_empty_when_prereqs_ok() {
        // When prerequisites are all Ok, the step must have no guided commands.
        use fdemon_daemon::toolchain::HostShell;
        let report = ToolchainReport {
            platform: HostPlatform::MacOs,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::Prerequisites,
                status: ComponentStatus::Ok,
                detail: "Xcode Command Line Tools and CocoaPods installed".to_string(),
            }],
            doctor: None,
            linux_package_manager: None,
            winget_available: false,
        };
        let steps = build_steps(&report);
        let prereq = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::Prerequisites)
            .expect("Prerequisites step must exist");
        assert!(
            prereq.guided_commands.is_empty(),
            "Prerequisites step must have no guided commands when all Ok"
        );
    }

    // ── Phase 5, Task 04: flutter_now_live + 9-ComponentKind routing ──────────

    #[test]
    fn test_flutter_now_live_returns_false_when_no_report() {
        let state = InstallWizardState::default();
        assert!(
            !state.flutter_now_live(),
            "flutter_now_live must return false when no report has been applied"
        );
    }

    #[test]
    fn test_flutter_now_live_returns_true_when_flutter_ok() {
        let mut state = InstallWizardState::default();
        let report = make_report(vec![make_check(
            ComponentKind::FlutterSdk,
            ComponentStatus::Ok,
        )]);
        state.apply_report(report);
        assert!(
            state.flutter_now_live(),
            "flutter_now_live must return true when FlutterSdk component is Ok"
        );
    }

    #[test]
    fn test_flutter_now_live_returns_false_when_flutter_missing() {
        let mut state = InstallWizardState::default();
        let report = make_report(vec![make_check(
            ComponentKind::FlutterSdk,
            ComponentStatus::Missing,
        )]);
        state.apply_report(report);
        assert!(
            !state.flutter_now_live(),
            "flutter_now_live must return false when FlutterSdk component is Missing"
        );
    }

    #[test]
    fn test_flutter_now_live_returns_false_when_no_flutter_component() {
        // Report has only Android components — no FlutterSdk entry.
        let mut state = InstallWizardState::default();
        let report = make_report(vec![make_check(
            ComponentKind::AndroidCmdlineTools,
            ComponentStatus::Ok,
        )]);
        state.apply_report(report);
        assert!(
            !state.flutter_now_live(),
            "flutter_now_live must return false when no FlutterSdk component is present"
        );
    }

    #[test]
    fn test_handback_done_defaults_to_false() {
        let state = InstallWizardState::default();
        assert!(!state.handback_done, "handback_done must default to false");
    }

    #[test]
    fn test_opening_resets_handback_done() {
        // `opening()` must reset handback_done so a re-opened wizard can hand back again.
        let state = InstallWizardState::opening(WizardOrigin::UserInvoked);
        assert!(
            !state.handback_done,
            "opening() must reset handback_done to false"
        );
    }

    /// Exhaustive test: all 9 `ComponentKind` variants must route to the correct
    /// `WizardStep` bucket in `build_steps()`.
    ///
    /// Routing rules (from `build_steps` match arm):
    /// - `Prerequisites`, `Git`                                    → `Prerequisites`
    /// - `AndroidCmdlineTools`, `AndroidPlatformTools`,
    ///   `AndroidPlatform`, `AndroidBuildTools`,
    ///   `AndroidLicenses`, `Jdk`                                  → `AndroidTools`
    /// - `FlutterSdk`                                              → `FlutterSdk`
    #[test]
    fn all_nine_component_kinds_route_to_correct_step() {
        // Build a report with one component of each kind, all Ok.
        let report = make_report(vec![
            make_check(ComponentKind::Prerequisites, ComponentStatus::Ok),
            make_check(ComponentKind::Git, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidCmdlineTools, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidPlatformTools, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidPlatform, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidBuildTools, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidLicenses, ComponentStatus::Ok),
            make_check(ComponentKind::Jdk, ComponentStatus::Ok),
            make_check(ComponentKind::FlutterSdk, ComponentStatus::Ok),
        ]);

        let steps = build_steps(&report);
        assert_eq!(
            steps.len(),
            5,
            "build_steps must always return exactly 5 steps"
        );

        let find_step = |kind: WizardStepKind| {
            steps
                .iter()
                .find(|s| s.kind == kind)
                .unwrap_or_else(|| panic!("{kind:?} step must be present"))
        };

        // Prerequisites step: Prerequisites + Git
        let prereq = find_step(WizardStepKind::Prerequisites);
        assert_eq!(
            prereq.components.len(),
            2,
            "Prerequisites step must have exactly 2 components (Prerequisites + Git)"
        );
        assert!(
            prereq
                .components
                .iter()
                .all(|c| matches!(c.kind, ComponentKind::Prerequisites | ComponentKind::Git)),
            "Prerequisites step must contain only Prerequisites and Git components"
        );

        // AndroidTools step: 6 components
        let android = find_step(WizardStepKind::AndroidTools);
        assert_eq!(
            android.components.len(),
            6,
            "AndroidTools step must have exactly 6 components"
        );
        for c in &android.components {
            assert!(
                matches!(
                    c.kind,
                    ComponentKind::AndroidCmdlineTools
                        | ComponentKind::AndroidPlatformTools
                        | ComponentKind::AndroidPlatform
                        | ComponentKind::AndroidBuildTools
                        | ComponentKind::AndroidLicenses
                        | ComponentKind::Jdk
                ),
                "component {:?} must not be in AndroidTools step",
                c.kind
            );
        }

        // PathConfig step: no components (informational)
        let path_cfg = find_step(WizardStepKind::PathConfig);
        assert!(
            path_cfg.components.is_empty(),
            "PathConfig step must have no components"
        );

        // FlutterSdk step: exactly 1 component
        let flutter = find_step(WizardStepKind::FlutterSdk);
        assert_eq!(
            flutter.components.len(),
            1,
            "FlutterSdk step must have exactly 1 component"
        );
        assert_eq!(
            flutter.components[0].kind,
            ComponentKind::FlutterSdk,
            "FlutterSdk step component must be FlutterSdk"
        );

        // Doctor step: no components (informational)
        let doctor = find_step(WizardStepKind::Doctor);
        assert!(
            doctor.components.is_empty(),
            "Doctor step must have no components"
        );
    }

    // ── Phase 5 Task 02: begin_step clears install_task and bumps run_seq ──────

    /// `begin_step` must clear any pre-existing `install_task` so a new run
    /// never inherits a stale handle from the previous step (F8).
    #[tokio::test]
    async fn test_begin_step_clears_pre_set_install_task() {
        let mut s = InstallWizardState::default();
        // Pre-set a task handle as if a previous run left one.
        let token = tokio_util::sync::CancellationToken::new();
        s.install_task = Some(InstallTaskHandle {
            cancel: token,
            join: Some(tokio::spawn(std::future::ready(()))),
        });
        assert!(s.install_task.is_some(), "precondition");

        s.begin_step(WizardStepKind::FlutterSdk);

        assert!(
            s.install_task.is_none(),
            "begin_step must clear install_task so the new run starts clean (F8)"
        );
    }

    /// `begin_step` must bump `run_seq` each time it is called.
    #[test]
    fn test_begin_step_bumps_run_seq() {
        let mut s = InstallWizardState::default();
        assert_eq!(s.run_seq, 0, "run_seq starts at 0");

        s.begin_step(WizardStepKind::FlutterSdk);
        assert_eq!(s.run_seq, 1, "first begin_step bumps to 1");

        s.begin_step(WizardStepKind::PathConfig);
        assert_eq!(s.run_seq, 2, "second begin_step bumps to 2");
    }

    // ── Bug 2: per-manager JDK guided command ─────────────────────────────────

    /// Helper: build a report with JDK missing on Linux with a specific PM.
    fn report_with_jdk_and_pm(pm: LinuxPackageManager) -> ToolchainReport {
        ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::Jdk,
                status: ComponentStatus::Missing,
                detail: String::new(),
            }],
            doctor: None,
            linux_package_manager: Some(pm),
            winget_available: false,
        }
    }

    #[test]
    fn test_jdk_command_uses_pacman_on_arch() {
        let report = report_with_jdk_and_pm(LinuxPackageManager::Pacman);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        let cmd = &android.guided_commands[0];
        assert_eq!(cmd.label, "Install JDK 17");
        assert!(
            cmd.command.contains("pacman"),
            "Pacman arm must use pacman; got: {}",
            cmd.command
        );
        assert!(
            cmd.command.contains("jdk17-openjdk"),
            "Pacman arm must install jdk17-openjdk; got: {}",
            cmd.command
        );
        assert!(
            cmd.note.is_some(),
            "Pacman arm must have a note (jre alternative)"
        );
    }

    #[test]
    fn test_jdk_command_uses_dnf_on_fedora() {
        let report = report_with_jdk_and_pm(LinuxPackageManager::Dnf);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        let cmd = &android.guided_commands[0];
        assert!(
            cmd.command.contains("dnf"),
            "Dnf arm must use dnf; got: {}",
            cmd.command
        );
        assert!(
            cmd.command.contains("java-17-openjdk-devel"),
            "Dnf arm must install java-17-openjdk-devel; got: {}",
            cmd.command
        );
    }

    #[test]
    fn test_jdk_command_uses_yum_on_rhel7() {
        let report = report_with_jdk_and_pm(LinuxPackageManager::Yum);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        let cmd = &android.guided_commands[0];
        assert!(
            cmd.command.contains("yum"),
            "Yum arm must use yum; got: {}",
            cmd.command
        );
        assert!(
            cmd.command.contains("java-17-openjdk-devel"),
            "Yum arm must install java-17-openjdk-devel; got: {}",
            cmd.command
        );
        assert!(
            !cmd.command.contains("dnf"),
            "Yum arm must not use dnf; got: {}",
            cmd.command
        );
    }

    #[test]
    fn test_jdk_command_uses_zypper_on_opensuse() {
        let report = report_with_jdk_and_pm(LinuxPackageManager::Zypper);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        let cmd = &android.guided_commands[0];
        assert!(
            cmd.command.contains("zypper"),
            "Zypper arm must use zypper; got: {}",
            cmd.command
        );
        assert!(
            cmd.command.contains("java-17-openjdk-devel"),
            "Zypper arm must install java-17-openjdk-devel; got: {}",
            cmd.command
        );
    }

    #[test]
    fn test_jdk_command_linux_unknown_pm_uses_adoptium_url() {
        let report = report_with_jdk_and_pm(LinuxPackageManager::Unknown);
        let steps = build_steps(&report);
        let android = steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap();
        let cmd = &android.guided_commands[0];
        assert!(
            cmd.command.contains("adoptium.net"),
            "Unknown PM must fall back to adoptium URL; got: {}",
            cmd.command
        );
        assert!(cmd.note.is_none(), "Unknown PM arm must have no note");
    }

    #[test]
    fn test_jdk_command_macos_windows_unchanged() {
        // macOS and Windows arms must not be affected by the Linux per-manager change.
        let mac_report = report_with_jdk(ComponentStatus::Missing, HostPlatform::MacOs);
        let steps = build_steps(&mac_report);
        let cmd = &steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap()
            .guided_commands[0];
        assert!(
            cmd.command.contains("brew"),
            "macOS arm must still use brew; got: {}",
            cmd.command
        );

        let win_report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Windows);
        let steps = build_steps(&win_report);
        let cmd = &steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .unwrap()
            .guided_commands[0];
        assert!(
            cmd.command.contains("winget"),
            "Windows arm must still use winget; got: {}",
            cmd.command
        );
    }

    // ── Bug 3: filtered Linux prerequisites command ───────────────────────────

    fn make_linux_report_with_pm_and_detail(
        pm: LinuxPackageManager,
        detail: &str,
        status: ComponentStatus,
    ) -> (ToolchainReport, Vec<ComponentCheck>) {
        let components = vec![ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status,
            detail: detail.to_string(),
        }];
        let report = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: components.clone(),
            doctor: None,
            linux_package_manager: Some(pm),
            winget_available: false,
        };
        (report, components)
    }

    #[test]
    fn test_linux_prereq_command_excludes_present_packages() {
        // detail says only clang and cmake are missing; curl and git should NOT appear.
        let (report, components) = make_linux_report_with_pm_and_detail(
            LinuxPackageManager::Apt,
            "missing: clang, cmake",
            ComponentStatus::Partial,
        );
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(cmds.len(), 1);
        let command = &cmds[0].command;
        assert!(
            command.contains("clang"),
            "clang must be in command; got: {command}"
        );
        assert!(
            command.contains("cmake"),
            "cmake must be in command; got: {command}"
        );
        assert!(
            !command.contains("curl"),
            "curl must NOT be in command (it is present); got: {command}"
        );
        assert!(
            !command.contains("git"),
            "git must NOT be in command (it is present); got: {command}"
        );
    }

    #[test]
    fn test_linux_prereq_command_empty_when_all_present() {
        // detail has no "missing:" prefix → all present → empty commands.
        let (report, components) = make_linux_report_with_pm_and_detail(
            LinuxPackageManager::Apt,
            "All required Linux tools present",
            ComponentStatus::Ok,
        );
        let cmds = prerequisites_guided_commands(&report, &components);
        assert!(
            cmds.is_empty(),
            "must return Vec::new() when no packages are missing; got: {cmds:?}"
        );
    }

    #[test]
    fn test_linux_prereq_package_names_mapped_per_manager_apt() {
        let (report, components) = make_linux_report_with_pm_and_detail(
            LinuxPackageManager::Apt,
            "missing: ninja, pkg-config, libgtk-3-dev, libglu1-mesa, libstdc++",
            ComponentStatus::Missing,
        );
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(cmds.len(), 1);
        let command = &cmds[0].command;
        assert!(
            command.contains("ninja-build"),
            "apt: ninja → ninja-build; got: {command}"
        );
        assert!(
            command.contains("pkg-config"),
            "apt: pkg-config stays; got: {command}"
        );
        assert!(
            command.contains("libgtk-3-dev"),
            "apt: gtk stays; got: {command}"
        );
        assert!(
            command.contains("libglu1-mesa"),
            "apt: glu stays; got: {command}"
        );
        assert!(
            command.contains("libstdc++-12-dev"),
            "apt: libstdc++ → libstdc++-12-dev; got: {command}"
        );
    }

    #[test]
    fn test_linux_prereq_package_names_mapped_per_manager_dnf() {
        let (report, components) = make_linux_report_with_pm_and_detail(
            LinuxPackageManager::Dnf,
            "missing: ninja, pkg-config, libgtk-3-dev, libglu1-mesa, libstdc++",
            ComponentStatus::Missing,
        );
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(cmds.len(), 1);
        let command = &cmds[0].command;
        assert!(
            command.contains("ninja-build"),
            "dnf: ninja → ninja-build; got: {command}"
        );
        assert!(
            command.contains("pkgconf"),
            "dnf: pkg-config → pkgconf; got: {command}"
        );
        assert!(
            command.contains("gtk3-devel"),
            "dnf: gtk → gtk3-devel; got: {command}"
        );
        assert!(
            command.contains("mesa-libGLU"),
            "dnf: glu → mesa-libGLU; got: {command}"
        );
        assert!(
            command.contains("libstdc++-devel"),
            "dnf: libstdc++ → libstdc++-devel; got: {command}"
        );
    }

    #[test]
    fn test_linux_prereq_package_names_mapped_per_manager_pacman() {
        let (report, components) = make_linux_report_with_pm_and_detail(
            LinuxPackageManager::Pacman,
            "missing: ninja, pkg-config, libgtk-3-dev, libglu1-mesa, libstdc++",
            ComponentStatus::Missing,
        );
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(cmds.len(), 1);
        let command = &cmds[0].command;
        assert!(
            command.contains("ninja"),
            "pacman: ninja stays; got: {command}"
        );
        // ninja-build must NOT appear (pacman uses 'ninja', not 'ninja-build')
        assert!(
            !command.contains("ninja-build"),
            "pacman must not use ninja-build; got: {command}"
        );
        assert!(
            command.contains("pkgconf"),
            "pacman: pkg-config → pkgconf; got: {command}"
        );
        assert!(
            command.contains("gtk3"),
            "pacman: gtk → gtk3; got: {command}"
        );
        assert!(
            command.contains("glu"),
            "pacman: glu stays as glu; got: {command}"
        );
        assert!(
            command.contains("gcc"),
            "pacman: libstdc++ → gcc; got: {command}"
        );
    }

    #[test]
    fn test_linux_prereq_xz_mapped_correctly_per_manager() {
        // xz maps to "xz-utils" on apt but "xz" on others.
        let (report_apt, comps_apt) = make_linux_report_with_pm_and_detail(
            LinuxPackageManager::Apt,
            "missing: xz",
            ComponentStatus::Partial,
        );
        let cmds = prerequisites_guided_commands(&report_apt, &comps_apt);
        assert_eq!(cmds.len(), 1);
        assert!(
            cmds[0].command.contains("xz-utils"),
            "apt: xz → xz-utils; got: {}",
            cmds[0].command
        );

        let (report_pacman, comps_pacman) = make_linux_report_with_pm_and_detail(
            LinuxPackageManager::Pacman,
            "missing: xz",
            ComponentStatus::Partial,
        );
        let cmds = prerequisites_guided_commands(&report_pacman, &comps_pacman);
        assert_eq!(cmds.len(), 1);
        assert!(
            cmds[0].command.contains(" xz"),
            "pacman: xz stays as xz; got: {}",
            cmds[0].command
        );
        assert!(
            !cmds[0].command.contains("xz-utils"),
            "pacman must not use xz-utils; got: {}",
            cmds[0].command
        );
    }

    #[test]
    fn test_linux_prereq_glu_and_libstdcpp_filtered_correctly() {
        // Both GLU and libstdc++ missing: both must appear in the command.
        let (report, components) = make_linux_report_with_pm_and_detail(
            LinuxPackageManager::Apt,
            "missing: libglu1-mesa, libstdc++",
            ComponentStatus::Partial,
        );
        let cmds = prerequisites_guided_commands(&report, &components);
        assert_eq!(cmds.len(), 1);
        let command = &cmds[0].command;
        assert!(
            command.contains("libglu1-mesa"),
            "libglu1-mesa must appear; got: {command}"
        );
        assert!(
            command.contains("libstdc++-12-dev"),
            "libstdc++ → libstdc++-12-dev must appear; got: {command}"
        );
        // curl and git are NOT in the missing list — must not appear.
        assert!(
            !command.contains("curl"),
            "curl must not appear; got: {command}"
        );
        assert!(
            !command.contains("git"),
            "git must not appear; got: {command}"
        );
    }

    // ── linux_package_name pure table tests ───────────────────────────────────

    #[test]
    fn test_linux_package_name_apt_mapping() {
        assert_eq!(
            linux_package_name("git", LinuxPackageManager::Apt),
            Some("git")
        );
        assert_eq!(
            linux_package_name("xz", LinuxPackageManager::Apt),
            Some("xz-utils")
        );
        assert_eq!(
            linux_package_name("ninja", LinuxPackageManager::Apt),
            Some("ninja-build")
        );
        assert_eq!(
            linux_package_name("pkg-config", LinuxPackageManager::Apt),
            Some("pkg-config")
        );
        assert_eq!(
            linux_package_name("libgtk-3-dev", LinuxPackageManager::Apt),
            Some("libgtk-3-dev")
        );
        assert_eq!(
            linux_package_name(PREREQ_KEY_GLU, LinuxPackageManager::Apt),
            Some("libglu1-mesa")
        );
        assert_eq!(
            linux_package_name(PREREQ_KEY_LIBSTDCPP, LinuxPackageManager::Apt),
            Some("libstdc++-12-dev")
        );
    }

    #[test]
    fn test_linux_package_name_dnf_mapping() {
        assert_eq!(
            linux_package_name("xz", LinuxPackageManager::Dnf),
            Some("xz")
        );
        assert_eq!(
            linux_package_name("ninja", LinuxPackageManager::Dnf),
            Some("ninja-build")
        );
        assert_eq!(
            linux_package_name("pkg-config", LinuxPackageManager::Dnf),
            Some("pkgconf")
        );
        assert_eq!(
            linux_package_name("libgtk-3-dev", LinuxPackageManager::Dnf),
            Some("gtk3-devel")
        );
        assert_eq!(
            linux_package_name(PREREQ_KEY_GLU, LinuxPackageManager::Dnf),
            Some("mesa-libGLU")
        );
        assert_eq!(
            linux_package_name(PREREQ_KEY_LIBSTDCPP, LinuxPackageManager::Dnf),
            Some("libstdc++-devel")
        );
    }

    #[test]
    fn test_linux_package_name_pacman_mapping() {
        assert_eq!(
            linux_package_name("xz", LinuxPackageManager::Pacman),
            Some("xz")
        );
        assert_eq!(
            linux_package_name("ninja", LinuxPackageManager::Pacman),
            Some("ninja")
        );
        assert_eq!(
            linux_package_name("pkg-config", LinuxPackageManager::Pacman),
            Some("pkgconf")
        );
        assert_eq!(
            linux_package_name("libgtk-3-dev", LinuxPackageManager::Pacman),
            Some("gtk3")
        );
        assert_eq!(
            linux_package_name(PREREQ_KEY_GLU, LinuxPackageManager::Pacman),
            Some("glu")
        );
        assert_eq!(
            linux_package_name(PREREQ_KEY_LIBSTDCPP, LinuxPackageManager::Pacman),
            Some("gcc")
        );
    }

    #[test]
    fn test_linux_package_name_unknown_returns_none() {
        assert_eq!(
            linux_package_name("git", LinuxPackageManager::Unknown),
            None
        );
        assert_eq!(
            linux_package_name("unknown-pkg", LinuxPackageManager::Apt),
            None
        );
    }
}
