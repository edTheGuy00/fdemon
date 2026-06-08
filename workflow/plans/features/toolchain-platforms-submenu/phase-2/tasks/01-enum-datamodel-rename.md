## Task: WizardStepKind variants + data model + AndroidTools→PlatformAndroid rename (compiling foundation)

**Objective**: Introduce the Platforms submenu data model as one compiling, test-green unit: add the new
`WizardStepKind` variants, rename `AndroidTools` → `PlatformAndroid`, add the `platforms_expanded` /
`indent` fields, rewrite `build_steps` to project a collapsed-or-expanded list with host-gated leaves,
and update every compiler-forced `match` arm and every affected test. No expand/collapse interactivity
or new rendering yet (Tasks 02 + 03) — but the wizard must compile and all existing tests must pass.

**Depends on**: Phase 1 (merged). Build on the post-Phase-1 order `Prerequisites → AndroidTools →
FlutterSdk → PathConfig → Doctor`.

**Agent:** implementor

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/types.rs` — `WizardStepKind` variants + `is_platform_leaf()`.
- `crates/fdemon-app/src/install_wizard/state.rs` — `WizardStep.indent`, `InstallWizardState.platforms_expanded`,
  `build_steps(report, expanded)` rewrite, parent rollup, `apply_report`, `Debug` impl, tests.
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` — rename in 3 match/if sites + add `Platforms`
  no-op arm + leaf "later phase" arms in `handle_run_selected_step`; tests.
- `crates/fdemon-app/src/actions/mod.rs` — rename `AndroidTools`→`PlatformAndroid` arm + add the 5 new
  variants to the non-executable catch-all; tests.
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` — renames (the `_` arms already cover new
  variants); `render_action_hint` parent guard; tests + `make_state_android_*` helpers.
- `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs` — **test helper only**: `make_steps()` adds
  `indent: 0` and renames `AndroidTools`→`PlatformAndroid` so the crate compiles. (Render code is Task 03.)

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `HostPlatform`, `ComponentKind`, `ToolchainReport.platform`.

### Details

> Line numbers are a pre-Phase-1 snapshot and will drift — locate by symbol/test-name/variant.

#### 1. `types.rs` — the enum

Rename `AndroidTools` → `PlatformAndroid`; add `Platforms`, `PlatformIos`, `PlatformMacos`,
`PlatformWeb`, `PlatformWindows`. Keep `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`. Add a helper:

```rust
impl WizardStepKind {
    /// True for the per-platform leaf rows nested under `Platforms`.
    pub fn is_platform_leaf(self) -> bool {
        matches!(self,
            WizardStepKind::PlatformAndroid | WizardStepKind::PlatformIos
            | WizardStepKind::PlatformMacos | WizardStepKind::PlatformWeb
            | WizardStepKind::PlatformWindows)
    }
}
```

Update the `GuidedCommand` doc-comment that references `AndroidTools` → `PlatformAndroid`.

#### 2. `state.rs` — data model + `build_steps`

- **`WizardStep`**: add `pub indent: u8,` (0 = top-level/parent, 1 = leaf). Populate it in every
  `WizardStep { … }` literal.
- **`InstallWizardState`**: add `pub platforms_expanded: bool,` (defaults `false`). Add it to the manual
  `Debug` impl. `opening()` picks it up via `..Self::default()`.
- **`build_steps` signature → `pub fn build_steps(report: &ToolchainReport, expanded: bool) -> Vec<WizardStep>`.**
  Project the visible list:
  - Always emit, in order: `Prerequisites` (0), **`Platforms` parent** (indent 0), then `FlutterSdk`,
    `PathConfig`, `Doctor`.
  - The Android component bucket (`AndroidCmdlineTools/PlatformTools/Platform/BuildTools/Licenses/Jdk`) +
    the JDK guided command move onto a **`PlatformAndroid` leaf** (indent 1, title e.g. `"Android"`).
  - When `expanded == true`, insert leaves **after the Platforms parent**, host-gated by `report.platform`:
    `PlatformAndroid` (all), `PlatformWeb` (all), `PlatformIos` + `PlatformMacos` (MacOs only),
    `PlatformWindows` (Windows only). In Phase 2, all leaves except `PlatformAndroid` are **placeholders**:
    empty `components`, `status: StepStatus::Pending`, no `guided_commands`, title e.g. `"iOS"`, `"Web"`, etc.
  - When `expanded == false`, emit **only** the parent (no leaves).
  - **Parent status**: roll up the leaf statuses, treating `Pending` (placeholders) as neutral — i.e. the
    parent reflects the Android leaf's real status in Phase 2. Add a small helper
    `fn rollup_step_statuses(&[StepStatus]) -> StepStatus` (Missing > Partial > Ok, ignore Pending; all
    Pending → Pending). The parent has empty `components` and no `guided_commands`.
- **`apply_report`**: call `build_steps(&report, self.platforms_expanded)`. The existing
  `selected_index >= len → 0` clamp and `selected_command_index = 0` reset are unchanged.
- Rename the local `android_tools` bucket var to `platform_android_components` (cosmetic). `is_jdk_actionable`
  stays as-is (pure `&[ComponentCheck]`); update only its doc-comment wording.

#### 3. `actions.rs` — forced match arms (compile) + rename

In `handle_run_selected_step`'s exhaustive `match kind`:
- Rename the `AndroidTools` arm → `PlatformAndroid` (keep its JDK gate via `is_jdk_actionable_from_state`
  and the `AndroidStepParams` dispatch unchanged).
- Add `WizardStepKind::Platforms => { /* parent: not executable */ UpdateResult::none() }`.
- Add `WizardStepKind::PlatformIos | PlatformMacos | PlatformWeb | PlatformWindows => { status_message =
  Some("Available in a later phase".into()); UpdateResult::none() }`.

In `handle_step_completed` (if-chains) and `handle_auto_configure_path` (`match` with `_`): rename
`AndroidTools` → `PlatformAndroid`. The `_` fallthrough covers the new variants correctly. Update the
doc-comments. (The `RunWizardStep` doc-comments in `handler/mod.rs` mention `AndroidTools` — those are in
Task 02's file; leave them, 02 renames them.)

#### 4. `actions/mod.rs` — executor catch-all (compile)

Rename the `AndroidTools` executor arm → `PlatformAndroid`. Extend the non-executable catch-all so it
covers every inert kind (the compiler will force this):

```rust
WizardStepKind::Prerequisites
| WizardStepKind::Doctor
| WizardStepKind::Platforms
| WizardStepKind::PlatformIos
| WizardStepKind::PlatformMacos
| WizardStepKind::PlatformWeb
| WizardStepKind::PlatformWindows => { /* existing WizardStepFailed "not executable" path */ }
```

#### 5. `step_detail.rs` — renames

Rename `AndroidTools` → `PlatformAndroid` in `step_caption`, `is_executable`, `action_hint_text`,
`render_action_hint`, the module doc-comment, and the `make_state_android_*` test helpers. The `_`
wildcards already handle the new variants. In `render_action_hint`, add `WizardStepKind::Platforms` to the
early-return guard (alongside `Doctor`) so the parent row shows no action hint.

#### 6. Tests (in 01's files)

Apply the rename `WizardStepKind::AndroidTools` → `PlatformAndroid` in all tests. Then:
- **Guided-command / JDK tests** that do `.find(|s| s.kind == AndroidTools)` must now build **expanded**
  (`build_steps(&report, true)`) and `.find(PlatformAndroid)` — the Android bucket + JDK command live on
  the leaf, which only exists when expanded.
- **`test_build_steps_produces_five_ordered_steps`**: update to `build_steps(&report, false)`; assert the
  5 collapsed kinds `[Prerequisites, Platforms, FlutterSdk, PathConfig, Doctor]`. Add a sibling test
  `…_expanded_inserts_android_leaf` asserting the expanded projection on a Linux report
  (`[Prerequisites, Platforms, PlatformAndroid, PlatformWeb, FlutterSdk, PathConfig, Doctor]`).
- **`len() == 5` assertions** (`test_apply_report_clears_loading_and_builds_steps`,
  `test_component_grouping_exhaustive`, etc.): collapsed default stays **5** — keep `== 5` where the state
  is collapsed. For the exhaustive-grouping test, assert the Android components land on the
  `PlatformAndroid` leaf (build expanded).
- Add **host-gating tests**: `build_steps(macos_report, true)` includes `PlatformIos` + `PlatformMacos`;
  `build_steps(windows_report, true)` includes `PlatformWindows`; neither appears on a Linux report. Use
  `make_report_for_platform`.
- Add a **parent-rollup test**: Android `Missing` → parent `Missing`; Android `Ok` + placeholders `Pending`
  → parent `Ok`.
- Fix any `~25` `build_steps(&report)` call sites to pass the new `expanded` arg (`false` unless the test
  needs leaves).

### Acceptance Criteria

1. `cargo build --workspace` compiles (both exhaustive matches updated).
2. `build_steps(r, false)` → 5 rows `[Prerequisites, Platforms, FlutterSdk, PathConfig, Doctor]`;
   `build_steps(r, true)` inserts host-gated leaves after `Platforms` (Android + Web always; iOS/macOS on
   MacOs; Windows on Windows).
3. The Platforms parent status equals the Android leaf status when other leaves are `Pending`
   placeholders.
4. `PlatformAndroid` keeps the JDK gate + `AndroidStepParams` dispatch; placeholder leaves return
   "Available in a later phase" and do not panic/hang.
5. No `WizardStepKind::AndroidTools` reference remains anywhere.
6. `cargo test --workspace --lib` green; `cargo fmt --all` + `cargo clippy --workspace -- -D warnings` clean.

### Testing

```bash
cargo build --workspace
cargo test -p fdemon-app --lib install_wizard
cargo test -p fdemon-app --lib handler::install_wizard
cargo test -p fdemon-tui --lib install_wizard
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

New tests to add: `build_steps_expanded_inserts_android_leaf`, host-gating (macOS/Windows/Linux leaf sets),
parent-status rollup.

### Notes

- **Do not** add the `InstallWizardToggleExpand` message, the toggle handler, Enter-routing, or any
  rendering caret/indent here — those are Tasks 02 and 03. In this task the parent is never expanded at
  runtime (no toggle yet), so leaves won't appear interactively; that is expected for this checkpoint.
- Keep `selected_command_index`, the JDK gate predicate, and the completion chain semantically unchanged —
  only names move.
- Host gating reads `report.platform` only (keep `build_steps` pure/testable).

---

## Completion Summary

**Status:** _(fill in)_
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
