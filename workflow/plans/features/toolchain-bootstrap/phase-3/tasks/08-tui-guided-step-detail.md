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

**Status:**
**Branch:**

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
