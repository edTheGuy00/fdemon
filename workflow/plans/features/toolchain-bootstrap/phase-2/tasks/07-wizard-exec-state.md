## Task: Wizard step execution state

**Objective**: Extend `InstallWizardState` with per-step execution state — running
flag, current phase label, download progress, a bounded streamed-log tail, and the
last result — so the TUI can render progress and so handlers can guard against
concurrent runs.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/types.rs`: add `StepExecStatus` and a
  `StepExecution` struct.
- `crates/fdemon-app/src/install_wizard/state.rs`: hold execution state on
  `InstallWizardState`, with mutators for the lifecycle messages.
- `crates/fdemon-app/src/install_wizard/mod.rs`: re-export the new types (and keep
  the existing daemon display-type re-exports intact).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/types.rs` (`WizardStepKind`, `StepStatus`).

### Details

```rust
/// Execution status of a single wizard step run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StepExecStatus {
    #[default]
    Idle,
    Running,
    Succeeded,
    Failed,
}

/// Live execution state for the step currently running (or last run).
#[derive(Debug, Clone, Default)]
pub struct StepExecution {
    pub kind: Option<WizardStepKind>,
    pub status: StepExecStatus,
    pub phase_label: Option<String>,        // "Cloning", "Downloading", "Precaching"
    pub received: u64,
    pub total: Option<u64>,
    pub log_tail: Vec<String>,              // bounded ring (see MAX_LOG_TAIL)
    pub result_summary: Option<String>,     // success summary or error message
}
```

Add to `InstallWizardState`:

```rust
/// Execution state for step runs (Phase 2+). Idle when nothing is running.
pub execution: StepExecution,
```

Mutators on `InstallWizardState` (called by task 09's handlers):

```rust
/// Whether a step is currently executing (guards re-entrancy).
pub fn is_step_running(&self) -> bool;

/// Begin a run: set Running, clear progress/log, set kind.
pub fn begin_step(&mut self, kind: WizardStepKind);

/// Record a streamed log line (bounded to MAX_LOG_TAIL lines).
pub fn push_step_log(&mut self, line: String);

/// Update download progress.
pub fn set_step_progress(&mut self, received: u64, total: Option<u64>);

/// Update the phase label.
pub fn set_step_phase(&mut self, label: String);

/// Finish a run (success or failure) with a summary.
pub fn finish_step(&mut self, status: StepExecStatus, summary: String);
```

Define `const MAX_LOG_TAIL: usize = 200;` (named constant, doc comment per
CODE_STANDARDS Principle 4). When the tail exceeds the cap, drop the oldest lines.

### Acceptance Criteria

1. `InstallWizardState::default()` has `execution` in `Idle` with an empty log tail.
2. `begin_step` sets `Running` and clears prior progress/log/summary.
3. `push_step_log` keeps at most `MAX_LOG_TAIL` lines, dropping the oldest.
4. `set_step_progress`/`set_step_phase` update the right fields without disturbing
   the log tail.
5. `finish_step(Succeeded|Failed, summary)` sets terminal status + summary and
   `is_step_running()` returns false afterward.
6. New types are re-exported from `install_wizard/mod.rs`. Unit-tested. No clippy warnings.

### Testing

```rust
#[test]
fn test_begin_step_sets_running_and_clears() { ... }

#[test]
fn test_log_tail_is_bounded() {
    let mut s = InstallWizardState::default();
    for i in 0..(MAX_LOG_TAIL + 50) { s.push_step_log(format!("line {i}")); }
    assert_eq!(s.execution.log_tail.len(), MAX_LOG_TAIL);
    assert!(s.execution.log_tail.first().unwrap().contains("line 50"));
}

#[test]
fn test_finish_step_sets_terminal_status() { ... }

#[test]
fn test_progress_updates_do_not_touch_log() { ... }
```

### Notes

- Keep `execution` separate from the per-step `StepStatus` rollup (which reflects
  preflight, not a live run). The TUI shows rollup status in the step list and
  execution state in the detail/progress pane.
- After a successful run, task 09 re-runs preflight; the rollup status then updates
  on its own — do not try to mutate `WizardStep.status` directly from execution.
- Preserve the manual `Debug` impl pattern already used for the `Cell` field; add
  `execution` to it.

---

## Completion Summary

**Status:** Not Started
</content>
