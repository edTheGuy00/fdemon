## Task: TUI — guided-command rendering in the step detail pane

**Objective**: Render the new `GuidedCommand` model in the install-wizard step
detail pane: show the guided command(s) for the selected step (e.g. the JDK install
command for Android Tools when JDK is missing), with a `c` copy hint and the
existing `Enter` run hint, plus a clear "JDK 17 required" note when the Android
Tools step is gated.

**Depends on**: 05

**Agent:** implementor

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`: render a
  guided-command block (label, command, optional note) and the `c`/`Enter` hints;
  show the JDK-required note for a gated Android Tools step.
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs`: only if a new sub-render
  entry point or layout slot is needed (keep minimal).

**Files Read (Dependencies):**
- `fdemon-app::install_wizard`: `GuidedCommand`, `WizardStep`, `WizardStepKind`,
  `StepStatus`, `StepExecution` (already re-exported for the TUI).
- existing `step_detail.rs` (Phase 2 layout + Enter action hint) and `progress.rs`
  (`StepProgress` — reused unchanged; Android install streams via the same
  `InstallEvent`/exec-state path).

### Details

The detail pane already renders the selected step's components and an Enter hint for
runnable steps (Phase 2). Add a **guided-command section** below the component list
when `step.guided_commands` is non-empty:

```
Guided steps (run these yourself, then press 'r' to re-check):

  Install JDK 17
    $ sudo apt install openjdk-17-jdk
    or: sudo dnf install java-17-openjdk-devel        [c] copy

[Enter] Install Android tools   (requires JDK 17)
```

Rendering rules:
- For each `GuidedCommand`: render `label`, the `command` on its own line
  (visually distinct, e.g. prefixed `$ ` or styled), and `note` if present.
- Show a `[c] copy` affordance next to the (first) guided command.
- For the **Android Tools** step specifically: if the step has a JDK guided command
  (i.e. JDK missing), render a one-line "JDK 17 required before installing Android
  tools" caption so the gate (task 07) is self-explanatory. When JDK is present and
  no guided command exists, render the normal `[Enter] Install Android tools` hint.
- Keep all content inside the allocated `Rect` using the `Layout` system (per the
  Responsive Layout Guidelines — no manual offset arithmetic); use a
  `Constraint::Min(0)` absorber.
- The live progress bar + streamed log tail during an Android install are already
  handled by the existing `StepProgress` widget (`progress.rs`) via the shared
  `execution` state — **no change needed there**.

### Acceptance Criteria

1. When the selected step has guided commands, the detail pane renders each
   command's label, command line, and optional note, plus a `[c] copy` hint.
2. For a gated Android Tools step (JDK missing), the pane shows a "JDK 17 required"
   caption and the JDK guided command; the `Enter`-to-run hint is not presented as
   the primary action.
3. When JDK is present, the Android Tools step shows the normal `[Enter]` run hint
   and no guided command.
4. All rendering stays within the allocated area (no overflow/panic) and uses named
   layout constants for any new thresholds.
5. `cargo check -p fdemon-tui` + widget render tests pass; a snapshot/return-based
   test covers the guided-command block.

### Testing

Follow the existing `step_detail` widget tests (render into a `TestTerminal`/buffer
and assert cell contents):

```rust
#[test]
fn test_detail_renders_jdk_guided_command() {
    // build a wizard state with AndroidTools selected + a JDK GuidedCommand
    // render step_detail into a test buffer
    // assert the buffer contains "Install JDK 17" and the command text and "copy"
}

#[test]
fn test_detail_android_enter_hint_when_jdk_present() {
    // no guided commands → assert the "[Enter] Install Android tools" hint renders
}
```

### Notes

- Reuse Phase 2's detail layout and styling conventions; this is additive.
- Don't re-implement progress/log rendering — `StepProgress` already covers the
  running state for any step that streams `InstallEvent`.
- Keep the guided-command rendering generic (driven by `step.guided_commands`), so
  Phase 4's prerequisite commands render through the same path with no further TUI
  work.
- Match the `c` copy affordance wording to the keybinding added in task 04.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-ab3140efe5f9d6740

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | Added guided-command rendering: `render_guided_commands()`, new layout constants (`GUIDED_SECTION_HEADER_HEIGHT`, `GUIDED_COMMAND_MIN_HEIGHT`, `JDK_CAPTION_HEIGHT`), updated `is_executable()` to handle `AndroidTools` based on `has_guided_commands`, added `AndroidTools` to `action_hint_text()`, updated `render_action_hint()` signature to accept `has_guided_commands`, updated `Widget::render` to use guided-command section instead of hint when step has guided commands, added 4 new tests |

### Notable Decisions/Tradeoffs

1. **AndroidTools executability**: `is_executable(AndroidTools, false)` = true (JDK present, run directly); `is_executable(AndroidTools, true)` = false (JDK missing, guided path). This matches task 07's JDK gate — the TUI mirrors the handler's gate decision by reading `has_guided_commands` rather than duplicating the JDK-check logic.

2. **Bottom-section space reservation**: When guided commands exist, the layout reserves `GUIDED_SECTION_HEADER_HEIGHT + JDK_CAPTION_HEIGHT (AndroidTools only) + GUIDED_COMMAND_MIN_HEIGHT` rows at the bottom of the content area. This keeps the component check rows in the upper portion without overlapping the guided command block.

3. **Updated existing test**: `test_step_detail_shows_phase_for_non_executable_step` was renamed and updated to `test_step_detail_shows_enter_hint_for_android_step_when_jdk_present`. The test previously verified "later phase" for AndroidTools; Phase 3 makes AndroidTools executable when JDK is present (no guided commands), so the expectation changed to "Press Enter".

4. **Generic rendering**: `render_guided_commands()` is driven purely by `step.guided_commands` and `step.kind` (for the AndroidTools-specific caption). Phase 4 prerequisites steps will render through the same path with no TUI changes needed.

### Testing Performed

- `cargo fmt --all -- --check` - PASS
- `cargo check --workspace --all-targets` - PASS
- `cargo test --workspace` - PASS (6571 tests, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` - PASS
- New tests added: `test_detail_renders_jdk_guided_command`, `test_detail_android_enter_hint_when_jdk_present`, `test_guided_command_with_note_renders_note`, `test_no_panic_guided_command_tiny_area`

### Risks/Limitations

1. **Task 07 not yet implemented**: The `handle_run_selected_step` AndroidTools arm and `InstallWizardCopyCommand` handler are not yet wired (task 07). The TUI renders the guided command and `[c] copy` affordance correctly, but pressing Enter on AndroidTools when JDK is present (no guided commands) will fall through to the existing "not handled" path until task 07 lands.
