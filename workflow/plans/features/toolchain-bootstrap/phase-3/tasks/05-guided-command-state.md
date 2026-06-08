## Task: Guided-command wizard state + build_steps derivation

**Objective**: Add a small, reusable `GuidedCommand` model to the install-wizard
state, derive the JDK guided-install command for the Android Tools step purely from
the preflight report + host platform in `build_steps()`, and expose a
`selected_guided_command()` accessor for the `c` copy key. This is the wizard's
first guided-command surface; Phase 4 will reuse it for all prerequisites.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/types.rs`: add `GuidedCommand`.
- `crates/fdemon-app/src/install_wizard/state.rs`: add `guided_commands` to
  `WizardStep` (or a derived accessor), populate them in `build_steps()`, and add
  `InstallWizardState::selected_guided_command()`.
- `crates/fdemon-app/src/install_wizard/mod.rs`: re-export `GuidedCommand`.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs`: `HostPlatform`, `ComponentStatus`,
  `ComponentKind`, `ToolchainReport` (re-exported via `fdemon-app::install_wizard`).
- existing `build_steps()`, `WizardStep`, `WizardStepKind` in `state.rs`/`types.rs`.

### Details

A guided command is a privileged/GUI step the wizard cannot auto-run — it is shown
to the user to copy/paste. For Phase 3 there is exactly one source: a missing/old
JDK on the Android Tools step.

```rust
/// A copy-paste command shown for a guided (privileged/GUI) step the wizard cannot
/// auto-run. Rendered in the detail pane and copyable with `c`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuidedCommand {
    pub label: String,    // e.g. "Install JDK 17"
    pub command: String,  // e.g. "sudo apt install openjdk-17-jdk"
    pub note: Option<String>, // e.g. "or: sudo dnf install java-17-openjdk-devel"
}
```

Attach guided commands to the relevant `WizardStep` (add
`pub guided_commands: Vec<GuidedCommand>` to `WizardStep`, default empty). In
`build_steps()`, when assembling the `AndroidTools` step, inspect the JDK component
(`ComponentKind::Jdk`): if its status is not `Ok` (i.e. `Missing`/`Partial`/`Error`),
push a `GuidedCommand` built from a small per-OS JDK command table owned here. The
JDK install command is a **display concern** (the wizard never auto-runs it), so it
lives in app-land — no dependency on the daemon's `jdk.rs`.

```rust
/// Per-OS guided command to install a JDK 17. Privileged → never auto-run; shown
/// for the user to copy/paste.
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
    GuidedCommand { label: "Install JDK 17".into(), command: command.into(), note: note.map(Into::into) }
}
```

```rust
impl InstallWizardState {
    /// The guided command the `c` key should copy: the first guided command of the
    /// currently selected step, if any.
    pub fn selected_guided_command(&self) -> Option<&GuidedCommand> {
        self.steps.get(self.selected_index)?.guided_commands.first()
    }
}
```

`build_steps()` receives the report; `HostPlatform` is available on
`ToolchainReport.platform`. Derivation is **pure** — no async, no I/O — so the
guided command appears as soon as preflight completes.

### Acceptance Criteria

1. `GuidedCommand` is defined, documented, and re-exported from
   `fdemon-app::install_wizard`.
2. `WizardStep` carries `guided_commands` (empty for steps without one).
3. After `build_steps()` with a report where JDK status is `Missing`, the
   `AndroidTools` step has one `GuidedCommand` whose `command` matches the host
   platform's JDK install command; when JDK status is `Ok`, it has none.
4. `selected_guided_command()` returns the selected step's first guided command, or
   `None`.
5. `cargo check -p fdemon-app` + `cargo test -p fdemon-app` pass.

### Testing

```rust
#[test]
fn test_android_step_has_jdk_guided_command_when_jdk_missing() {
    let report = report_with_jdk(ComponentStatus::Missing, HostPlatform::Linux);
    let steps = build_steps(&report);
    let android = steps.iter().find(|s| s.kind == WizardStepKind::AndroidTools).unwrap();
    assert_eq!(android.guided_commands.len(), 1);
    assert!(android.guided_commands[0].command.contains("17"));
}

#[test]
fn test_no_guided_command_when_jdk_ok() {
    let report = report_with_jdk(ComponentStatus::Ok, HostPlatform::Linux);
    let steps = build_steps(&report);
    let android = steps.iter().find(|s| s.kind == WizardStepKind::AndroidTools).unwrap();
    assert!(android.guided_commands.is_empty());
}
```

### Notes

- Keep the model minimal and reusable — Phase 4 will populate `guided_commands` for
  the `Prerequisites` step (apt/brew/xcode-select/Rosetta/CocoaPods). Do not
  hardcode JDK-specific assumptions into `GuidedCommand` itself.
- Derivation must be pure: read `report.components` for `Jdk` status and
  `report.platform`. No process spawning in `build_steps()`.
- **No daemon coupling:** the per-OS JDK command table lives here (display concern),
  so this task has no dependency on task 02. The daemon's `jdk.rs` only handles
  `resolve_jdk_home`/`configure_flutter_jdk_dir`, which are not display strings.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/types.rs` | Added `GuidedCommand` struct with `label`, `command`, `note` fields; added 3 unit tests |
| `crates/fdemon-app/src/install_wizard/state.rs` | Added `guided_commands: Vec<GuidedCommand>` to `WizardStep`; added `jdk_guided_command(platform)` helper; updated `build_steps()` to populate guided commands when JDK status is not Ok; added `selected_guided_command()` to `InstallWizardState`; added `HostPlatform` import; added 14 unit tests covering all acceptance criteria |
| `crates/fdemon-app/src/install_wizard/mod.rs` | Re-exported `GuidedCommand` from public API |

### Notable Decisions/Tradeoffs

1. **Pure derivation**: `jdk_guided_command()` is a free function taking `HostPlatform` by value (cloned from `report.platform`). No I/O, no process spawning — the guided command appears as soon as preflight completes.
2. **JDK Unknown status not triggering guided command**: Only `Missing`, `Partial`, and `Error` statuses trigger the guided command (all non-`Ok` values that actually include a `Jdk` component). `Unknown` is treated as "skip was skipped" (prerequisite missing), which doesn't warrant a JDK install suggestion since Android SDK root is not even known.
3. **`guided_commands` default is empty `Vec`**: All five wizard steps get the field; non-Android steps default to `Vec::new()`. This keeps `WizardStep` struct-update syntax and `Default` derivation straightforward.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check -p fdemon-app` — Passed
- `cargo test -p fdemon-app` — Passed (2737 tests, 0 failed)
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — Passed

New tests added:
- `test_android_step_has_jdk_guided_command_when_jdk_missing` — matches task spec exactly
- `test_no_guided_command_when_jdk_ok` — matches task spec exactly
- `test_android_step_has_jdk_guided_command_when_jdk_partial` — edge case
- `test_android_step_has_jdk_guided_command_when_jdk_error` — edge case
- `test_jdk_command_linux_contains_apt` — platform-specific command verification
- `test_jdk_command_macos_uses_brew` — platform-specific command verification
- `test_jdk_command_windows_uses_winget` — platform-specific command verification
- `test_jdk_command_unknown_platform_uses_adoptium` — platform-specific command verification
- `test_non_android_steps_have_no_guided_commands` — all other steps clean
- `test_selected_guided_command_returns_none_when_no_steps` — accessor edge case
- `test_selected_guided_command_returns_none_when_step_has_none` — accessor with empty step
- `test_selected_guided_command_returns_first_when_android_selected` — accessor happy path
- 3 tests for `GuidedCommand` struct in `types.rs`

### Risks/Limitations

1. **Phase 4 prerequisites**: When Phase 4 adds guided commands for the `Prerequisites` step, `build_steps()` will need a parallel derivation block. The `GuidedCommand` struct is intentionally minimal and generic — no JDK-specific fields — to facilitate reuse.
