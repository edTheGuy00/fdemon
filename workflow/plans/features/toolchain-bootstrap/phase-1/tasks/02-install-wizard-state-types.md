## Task: InstallWizard State Types (fdemon-app, new files only)

**Objective**: Add the `install_wizard/` feature module to `fdemon-app` holding the read-only
wizard state types (`InstallWizardState`, `WizardStep`, `WizardStepKind`, `StepStatus`,
`WizardPane`) and the `build_steps()` mapper that turns a daemon `ToolchainReport` into ordered UI
steps. This task adds **only new files plus `lib.rs` module declarations** so it compiles
standalone with no enum/match changes elsewhere.

**Depends on**: 01-toolchain-preflight-subsystem (for `ToolchainReport`/`ComponentCheck`)

**Agent:** implementor

**Estimated Time**: 4-5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/mod.rs` (NEW) — `pub use state::*; pub use types::*;`.
- `crates/fdemon-app/src/install_wizard/types.rs` (NEW) — `WizardPane`, `WizardStepKind`, `StepStatus`.
- `crates/fdemon-app/src/install_wizard/state.rs` (NEW) — `InstallWizardState`, `WizardStep`,
  `build_steps()`.
- `crates/fdemon-app/src/lib.rs` — add `pub mod install_wizard;`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/flutter_version/{state.rs,types.rs,mod.rs}` — structural template
  (`FlutterVersionState`, `FlutterVersionPane`, `VersionListState` with `Cell<usize>` render-hint).
- Task 01 `toolchain` types (`ToolchainReport`, `ComponentCheck`, `ComponentStatus`, `ComponentKind`,
  `DoctorLine`), accessed via `fdemon_daemon::toolchain::...` / `fdemon_daemon::...`.

### Details

Mirror `flutter_version/` exactly. `mod.rs` is re-exports only.

**`types.rs`:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WizardPane { #[default] StepList, Detail }

/// User-facing ordered steps (the install dependency order is handled later).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardStepKind { Prerequisites, AndroidTools, PathConfig, FlutterSdk, Doctor }

/// Per-step roll-up status derived from the underlying component checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus { Ok, Partial, Missing, Pending }
```

> `StepStatus` has no `Running`/`Failed` in Phase 1 (no step execution). `Pending` = preflight not
> yet completed.

**`state.rs`:**

```rust
use std::cell::Cell;
use fdemon_daemon::toolchain::{ToolchainReport, ComponentCheck};
use super::types::{WizardPane, WizardStepKind, StepStatus};

#[derive(Debug, Clone)]
pub struct WizardStep {
    pub kind: WizardStepKind,
    pub title: String,
    pub status: StepStatus,
    /// Component checks rolled into this step (rendered in the detail pane).
    pub components: Vec<ComponentCheck>,
}

#[derive(Debug, Default)]
pub struct InstallWizardState {
    pub visible: bool,
    pub focused_pane: WizardPane,
    pub steps: Vec<WizardStep>,
    pub selected_index: usize,
    /// Detail-pane vertical scroll (incl. embedded doctor view).
    pub detail_scroll: usize,
    pub report: Option<ToolchainReport>,
    /// True while a preflight task is in flight (initial open + `r` re-run).
    pub loading: bool,
    pub status_message: Option<String>,
    /// Render-hint: detail-pane visible height from the last frame (TEA Cell exception).
    pub last_known_visible_height: Cell<usize>,
}

impl InstallWizardState {
    /// Fresh state for opening the wizard; preflight has not completed yet.
    pub fn opening() -> Self {
        Self { visible: true, loading: true, ..Self::default() }
    }

    /// Populate steps from a completed preflight report.
    pub fn apply_report(&mut self, report: ToolchainReport) {
        self.steps = build_steps(&report);
        self.report = Some(report);
        self.loading = false;
        if self.selected_index >= self.steps.len() { self.selected_index = 0; }
    }

    pub fn selected_step(&self) -> Option<&WizardStep> { self.steps.get(self.selected_index) }
}

/// Map a ToolchainReport's components into the five ordered UI steps.
pub fn build_steps(report: &ToolchainReport) -> Vec<WizardStep> { /* group + roll up status */ }
```

- `build_steps` groups `report.components` by `WizardStepKind`:
  - `Prerequisites` ← `ComponentKind::Prerequisites`
  - `AndroidTools` ← `AndroidCmdlineTools, AndroidPlatformTools, AndroidPlatform, AndroidBuildTools, AndroidLicenses, Jdk`
  - `PathConfig` ← (Phase 1: a static informational step; no component — status `Pending`/`Ok`
    based on whether Flutter is resolved). Keep minimal.
  - `FlutterSdk` ← `ComponentKind::FlutterSdk`
  - `Doctor` ← no component; its detail is `report.doctor`.
- Per-step `status` roll-up: `Missing` if any child is `Missing`; else `Partial` if any is
  `Partial`/`Error`; else `Ok`. Empty/no-component informational steps → `Ok` or `Pending`.

### Acceptance Criteria

1. `fdemon_app::install_wizard::InstallWizardState` (and the enums) are public and `Default`-able.
2. `InstallWizardState::default()` has `visible == false`, `loading == false`, `steps` empty.
3. `apply_report` builds exactly the five ordered steps and clears `loading`.
4. `build_steps` roll-up: a step containing one `Missing` component reports `StepStatus::Missing`;
   all-`Ok` components report `StepStatus::Ok`.
5. The crate compiles with no changes to `state.rs`, `message.rs`, or any `match` (new files +
   `lib.rs` declaration only).

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opening_state_is_visible_and_loading() {
        let s = InstallWizardState::opening();
        assert!(s.visible && s.loading && s.steps.is_empty());
    }

    #[test]
    fn test_build_steps_produces_five_ordered_steps() { /* assert kinds in order */ }

    #[test]
    fn test_step_status_rollup_missing_wins() { /* one Missing child -> Missing */ }
}
```

- Build small `ToolchainReport` fixtures (construct `ComponentCheck` values directly) and assert the
  derived step ordering and status roll-up.

### Notes

- The `Cell<usize>` render-hint must carry the `// EXCEPTION: TEA render-hint write-back via Cell`
  annotation **at its write site** (in the TUI task), per CODE_STANDARDS Principle 3. Here it is
  only declared.
- Do not add navigation/scroll *logic* here — that lives in the handler (task 03). This task is
  pure data + the `build_steps` mapper.
