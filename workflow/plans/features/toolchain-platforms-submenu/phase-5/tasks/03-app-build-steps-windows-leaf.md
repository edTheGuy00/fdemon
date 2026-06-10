## Task: App build_steps — live Windows leaf (bucket + cap + `windows_guided_commands`)

**Objective**: Replace Task 01's no-op stub arm and the Phase-2 placeholder Windows leaf in
`build_steps` with the real thing: a `platform_windows_components` bucket for
`ComponentKind::VisualStudioCpp`, the Missing→Partial non-blocking cap, a new
`windows_guided_commands` builder (winget / choco / modify-existing-VS), and the live leaf body.
`install_wizard/state.rs` only.

**Depends on**: Task 01 (merged — `ComponentKind::VisualStudioCpp` + stub arm exist).
Runs in parallel with Task 02 (write-disjoint).

**Agent:** implementor

**Complexity:** medium

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentKind::VisualStudioCpp`,
  `ToolchainReport.winget_available`, the detail-prefix constant from Task 01 (`"Visual Studio found"`).
- `crates/fdemon-app/src/install_wizard/types.rs` — `GuidedCommand`, `StepStatus`.
- In-file analogs: `web_browser_guided_commands` (winget branching on `report.winget_available`),
  `xcode_guided_commands` (status-gated early-out), the `web_status` / `ios_status` cap blocks.

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/test-name/variant.

#### 1. Component routing — replace the stub arm

In the `build_steps` component-routing match (~`state.rs:1206`), replace Task 01's
`ComponentKind::VisualStudioCpp => {}` stub with:

```rust
ComponentKind::VisualStudioCpp => {
    platform_windows_components.push(check.clone());
}
```

declaring `platform_windows_components` beside the other buckets.

#### 2. Status — Missing→Partial cap

Mirror the `web_status` block verbatim (including its comment style):

```rust
// Windows status: non-blocking cap — `Missing` becomes `Partial` so that an absent
// Visual Studio never propagates `Missing` up through the Platforms parent.
// Empty (no VisualStudioCpp component, i.e. non-Windows report) → Pending.
let windows_status = if platform_windows_components.is_empty() {
    StepStatus::Pending
} else {
    let raw = rollup_status(&platform_windows_components);
    if raw == StepStatus::Missing { StepStatus::Partial } else { raw }
};
```

#### 3. `windows_guided_commands` builder (NEW)

```rust
/// Guided commands for the Windows platform leaf. Emits only when the leaf is
/// `Partial` (the capped form of a determined-absent VS C++ workload); `Ok` and
/// `Pending` yield no commands. Display-only copy-paste text — never executed.
fn windows_guided_commands(
    report: &ToolchainReport,
    status: StepStatus,
    components: &[ComponentCheck],
) -> Vec<GuidedCommand>
```

- `if status != StepStatus::Partial { return Vec::new(); }` (the `xcode_guided_commands` early-out).
- **Branch on the detail-prefix contract**: if the `VisualStudioCpp` component's `detail` starts with
  `"Visual Studio found"` (Task 01's stable prefix — add a comment pointing at
  `checks/windows.rs::classify_vswhere_gates`), VS exists but the C++ workload is missing → emit the
  **modify** entry first:
  - label `"Add the C++ workload to the existing Visual Studio"`, command
    `start "" "%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\setup.exe"`, note
    `"Opens Visual Studio Installer — choose Modify and tick 'Desktop development with C++'."`
- **Fresh-install entries** (emitted in both branches; primary in the no-VS branch):
  - When `report.winget_available`: label `"Install VS 2022 Build Tools (winget)"`, command
    `winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.NativeDesktop;includeRecommended"`,
    note about elevation + restart of the wizard re-check (`r`).
  - Always: label `"Install VS 2022 Build Tools (choco)"`, command
    `choco install visualstudio2022buildtools --package-parameters "--add Microsoft.VisualStudio.Workload.NativeDesktop --includeRecommended"`,
    note `"Requires Chocolatey."`
- Keep ordering deterministic (modify → winget → choco) so `[`/`]` cycling and tests are stable. Follow
  the exact quoting style above — the `--override` payload is one quoted string.

#### 4. Leaf body — replace the placeholder

Replace the placeholder leaf (~`state.rs:1382`):

```rust
if report.platform == HostPlatform::Windows {
    leaves.push(WizardStep {
        kind: WizardStepKind::PlatformWindows,
        title: "Windows".to_string(),
        status: windows_status,
        components: platform_windows_components.clone(),
        guided_commands: windows_guided_commands(report, windows_status, &platform_windows_components),
        indent: 1,
    });
}
```

Host gating stays `report.platform == HostPlatform::Windows` (runtime, from the report — not `cfg!`).
Update the `build_steps` doc-comment (the per-leaf routing list, ~`state.rs:1169`) to record
`PlatformWindows` — `ComponentKind::VisualStudioCpp`, status capped at `Partial`.

### Acceptance Criteria

1. A Windows report carrying a `VisualStudioCpp` check yields a `PlatformWindows` leaf with that
   component, status `Ok` (check Ok) / `Partial` (check Missing — capped), and guided commands exactly
   when `Partial`.
2. The "VS found, workload missing" detail prefix produces the modify entry first; a plain
   "not found" detail produces winget (when available) + choco only.
3. `winget_available == false` suppresses the winget entry; choco remains.
4. Non-Windows reports: leaf absent, bucket empty, no behaviour change to any other leaf or the
   Platforms-parent rollup (existing Linux-report tests stay green untouched except where they assert
   the old placeholder `Pending` status on Windows reports).
5. A Windows report with VS missing still rolls the Platforms parent up to at most `Partial` (never
   `Missing`) and does not affect `flutter_now_live()` / `all_components_ok()` semantics.
6. `cargo test -p fdemon-app --lib` green; `cargo fmt --all` + `cargo clippy --workspace -- -D warnings`
   clean.

### Testing

```bash
cargo test -p fdemon-app --lib install_wizard
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

New tests (mirror the Phase-3 web / Phase-4 ios test names):
- `test_build_steps_windows_leaf_routes_visualstudio_component` — Windows report + `VisualStudioCpp`
  check → leaf has 1 component.
- `test_build_steps_windows_leaf_caps_missing_to_partial` — Missing check → leaf `Partial`.
- `test_build_steps_windows_leaf_ok_when_component_ok` — Ok check → leaf `Ok`, empty guided commands.
- `test_build_steps_windows_leaf_empty_components_pending` — Windows report without the component →
  `Pending` (legacy-report safety).
- `test_windows_guided_commands_fresh_install_winget_and_choco` / `…_no_winget_falls_back_to_choco` /
  `…_existing_vs_emits_modify_first` / `…_ok_status_empty` / `…_pending_status_empty`.
- `test_build_steps_windows_parent_rollup_partial_not_missing` — parent rollup over leaves stays
  `Partial`.
- Update any existing Windows-report test asserting the placeholder (`Pending`, empty components) to
  the live shape; presence/absence-by-host tests (~`state.rs:4179`) remain valid as-is.

### Notes

- **Do not touch `actions.rs`** — Task 02 owns it this wave.
- **Detail-prefix contract**: match with `starts_with` against a local `const` that duplicates Task
  01's string, with cross-referencing comments at both sites (the crates don't share a visibility path
  for it — if Task 01 exported a `pub` const from `fdemon-daemon`, prefer importing that instead of
  duplicating). Prefix drift degrades to "fresh-install commands shown" — acceptable, not a panic path.
- **Commands are display-only** — verbatim copy-paste text with notes; fdemon never executes them.
- `all_components_ok()` stays strict (Phase 3/4 precedent): a Windows host without VS doesn't show
  "All set". Do not special-case `VisualStudioCpp` out of it.
