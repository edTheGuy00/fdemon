## Task: Wizard step-execution protocol (messages, action, key binding)

**Objective**: Define the TEA plumbing for running a wizard step: the
`RunWizardStep` `UpdateAction`, the step lifecycle/progress/log/completion
`Message` variants, and the `Enter` key binding that triggers a step run.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/message.rs`: add the wizard step `Message` variants.
- `crates/fdemon-app/src/handler/mod.rs`: add the `UpdateAction::RunWizardStep`
  variant.
- `crates/fdemon-app/src/handler/keys.rs`: bind `Enter` in `UiMode::InstallWizard`
  (currently intentionally unbound — see the Phase 1 comment near line 419).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/types.rs`: `WizardStepKind` (already exists).
- `crates/fdemon-app/src/message.rs` flutter_version variants
  (`FlutterVersionSwitchCompleted`, etc.) as the naming/shape template.

### Details

New `Message` variants (place under the existing `// ── Install Wizard ──` block):

```rust
/// Run (or retry) the currently selected wizard step.
InstallWizardRunSelectedStep,
/// A wizard step has started executing.
WizardStepStarted { kind: WizardStepKind },
/// Streamed log line from a running wizard step.
WizardStepLog { kind: WizardStepKind, line: String },
/// Download progress for a running wizard step.
WizardDownloadProgress { kind: WizardStepKind, received: u64, total: Option<u64> },
/// A wizard step finished successfully (carries a human-readable summary, e.g.
/// the resolved SDK path or the rc file written).
WizardStepCompleted { kind: WizardStepKind, summary: String, sdk_path: Option<std::path::PathBuf> },
/// A wizard step failed.
WizardStepFailed { kind: WizardStepKind, reason: String },
```

`WizardStepKind` is re-exported from `crate::install_wizard`; import it in
`message.rs` the same way other module enums are imported.

New `UpdateAction` variant (place under the existing `// ── Install Wizard ──`
block near `RunToolchainPreflight`):

```rust
/// Execute a wizard step asynchronously (Flutter SDK install or PATH config).
/// Emits WizardStepStarted/Log/DownloadProgress and WizardStepCompleted|Failed.
RunWizardStep {
    kind: WizardStepKind,
    /// Resolved Flutter install parameters (None for the PathConfig step).
    install: Option<FlutterStepParams>,
    /// Flutter bin dir to add to PATH (Some for the PathConfig step).
    path_bin_dir: Option<std::path::PathBuf>,
},
```

Where `FlutterStepParams` is a small app-side struct (define in `handler/mod.rs`
near the action, or reuse fields inline) carrying what task 08 needs to build a
`fdemon_daemon::toolchain::FlutterInstallTarget`:

```rust
#[derive(Debug, Clone)]
pub struct FlutterStepParams {
    pub method: fdemon_daemon::toolchain::InstallMethod,
    pub channel: String,
    pub install_root: Option<std::path::PathBuf>, // None → daemon resolves default
}
```

(If `InstallMethod` isn't yet re-exported from `fdemon_daemon::toolchain`, add the
re-export as part of task 03's `mod.rs`; this task only references the type.)

Key binding in `keys.rs` `handle_key_install_wizard`:

```rust
InputKey::Enter => Some(Message::InstallWizardRunSelectedStep),
```

Update the Phase-1 doc comment that says "`Enter` is intentionally unbound in
Phase 1 (step execution is Phase 2)" to reflect the new binding.

### Acceptance Criteria

1. The new `Message` variants and `UpdateAction::RunWizardStep` compile and are
   exhaustively matchable (no `_` catch-all added that would hide them).
2. `Enter` in `UiMode::InstallWizard` produces `Message::InstallWizardRunSelectedStep`.
3. `cargo check -p fdemon-app` passes; existing `handler::update` match remains
   exhaustive (the actual handling lands in task 09 — until then, add a minimal
   arm or stub so the crate compiles, clearly marked for task 09 to flesh out).
4. A `keys.rs` unit test asserts the `Enter` mapping.

### Testing

```rust
#[test]
fn test_enter_in_install_wizard_runs_selected_step() {
    // handle_key_install_wizard(Enter, &state) == Some(InstallWizardRunSelectedStep)
}
```

### Notes

- This task only introduces the protocol; the executor (task 08) and the handlers
  (task 09) consume it. Coordinate the stub arms so the crate stays compiling
  between tasks — prefer minimal `UpdateResult::none()` placeholders annotated
  `// TODO(phase2-task-09)`.
- Keep message field names aligned with the daemon `InstallEvent` mapping so task
  08's bridge is mechanical.

---

## Completion Summary

**Status:** Not Started
</content>
