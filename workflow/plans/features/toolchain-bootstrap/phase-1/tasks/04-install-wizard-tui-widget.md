## Task: InstallWizard TUI Widget (fdemon-tui)

**Objective**: Build the read-only wizard widget: a two-pane modal (ordered step list + per-step
detail) with an embedded `flutter doctor` view, mirroring `widgets/flutter_version_panel/`. Renders
purely from `&InstallWizardState`; no state mutation except the `Cell<usize>` render-hint.

**Depends on**: 02-install-wizard-state-types (state types only — can run in parallel with task 03)

**Agent:** implementor

**Estimated Time**: 6-8 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` (NEW) — `InstallWizardPanel<'a>` orchestrator.
- `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs` (NEW) — left pane.
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` (NEW) — right pane.
- `crates/fdemon-tui/src/widgets/install_wizard/doctor_view.rs` (NEW) — embedded doctor output.
- `crates/fdemon-tui/src/widgets/mod.rs` — `pub mod install_wizard;` + `pub use install_wizard::InstallWizardPanel;`.

**Files Read (Dependencies):**
- Task 02 state types (`InstallWizardState`, `WizardStep`, `WizardStepKind`, `StepStatus`, `WizardPane`).
- `fdemon_daemon::toolchain::{ComponentCheck, ComponentStatus, DoctorLine, DoctorMarker}`.
- `crates/fdemon-tui/src/widgets/flutter_version_panel/{mod.rs,sdk_info.rs,version_list.rs}` —
  template (background dim, centered rect via `modal_overlay::centered_rect_percent`,
  horizontal/vertical pane split, render-hint `Cell` write).

### Details

**`mod.rs` — `InstallWizardPanel<'a>` (implements `Widget`):**
- Mirror `FlutterVersionPanel`: dim background, compute centered rect, draw bordered block, split
  outer area into `header(3) | sep(1) | panes(flex) | sep(1) | footer(1) | absorber(0)`.
- Footer renders the Phase 1 key hints: `Tab` switch · `j/k` move · `r` re-run · `Esc` close.
- `loading == true` → render a centered "Running preflight checks…" placeholder in the panes area
  (skip the two-pane split while loading).
- Pane split: horizontal when `inner.width >= MIN_HORIZONTAL_WIDTH` (reuse the flutter-version
  threshold, named constant with derivation comment), else vertical. Left = step list, right =
  detail.
- Highlight the focused pane (border/title style) from `state.focused_pane`.

**`step_list.rs` — `StepListPane<'a>`:**
- One row per `WizardStep`: a status glyph + `title`. Glyph/style by `StepStatus`:
  `Ok → ✓ green`, `Partial → ! yellow`, `Missing → ✗ red`, `Pending → … dim`.
- Highlight `selected_index`. Use existing palette constants (`palette::STATUS_RED`, etc.).

**`step_detail.rs` — `StepDetailPane<'a>`:**
- Renders the `selected_step()`:
  - For `WizardStepKind::Doctor`, delegate to `DoctorView` (render `state.report.doctor`).
  - Otherwise, render the step's `components`: each `ComponentCheck` as a line
    `<glyph> <kind label>: <detail>` colored by `ComponentStatus`.
- Vertical scroll via `state.detail_scroll`; write the actual visible height back to
  `state.last_known_visible_height` each frame:

```rust
// EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
self.state.last_known_visible_height.set(visible_height);
```

  Add a render-time scroll clamp (safety net) so the content stays in view (mirror
  `version_list.rs`).

**`doctor_view.rs` — `DoctorView<'a>`:**
- Input: `&[DoctorLine]` (or `Option<&Vec<DoctorLine>>`). When `None`, render
  "flutter doctor unavailable (Flutter not installed)".
- One styled line per `DoctorLine`, indented by `indent`, colored by `DoctorMarker`
  (`Ok → green`, `Warning → yellow`, `Error/Dead → red`, `None → default`). Prefix with the marker
  glyph for marked lines.

### Acceptance Criteria

1. `widgets::InstallWizardPanel::new(&state.install_wizard_state)` constructs and renders without
   panicking for: loading state, empty steps, populated steps, and a step with no components.
2. Step list shows the correct glyph/color per `StepStatus`; the selected step is visually
   highlighted.
3. Selecting the Doctor step renders the parsed `flutter doctor` lines with per-marker coloring;
   when `report.doctor` is `None`, the unavailable placeholder is shown.
4. The detail pane writes `last_known_visible_height` each frame (annotated) and clamps scroll so
   the content remains visible.
5. Horizontal/vertical pane split is chosen by available width via a named threshold constant.

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestTerminal; // or buffer-based assertions

    #[test] fn test_renders_loading_placeholder() { /* loading=true */ }
    #[test] fn test_renders_step_list_with_status_glyphs() { /* */ }
    #[test] fn test_doctor_view_renders_markers() { /* DoctorLine fixtures */ }
    #[test] fn test_doctor_view_none_shows_unavailable() { /* */ }
    #[test] fn test_detail_pane_writes_visible_height_hint() { /* assert Cell set */ }
}
```

- Use the project's buffer/snapshot test style (see `widgets/flutter_version_panel` tests and
  `test_utils::TestTerminal`). Build `InstallWizardState` fixtures directly.

### Notes

- This task uses the simplest render path (`frame.render_widget`, like `FlutterVersionPanel`) — no
  `MouseCtx`/region plumbing is needed (the wizard is keyboard + scroll only). The render-branch
  wiring lives in task 05.
- Keep each file under ~500 lines; split a pane into helpers if it grows.
- Do not reference any Phase 2 concepts (progress bars, step execution) — `progress.rs` is a Phase 2
  file and is **not** created here.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-ae2ed166494230d85

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` | NEW — `InstallWizardPanel<'a>` orchestrator: dim background, centered overlay, header (title + close hint), horizontal/vertical pane layout switching by `MIN_HORIZONTAL_WIDTH`, loading placeholder, footer with Phase 1 key hints, named layout constants with derivation comments |
| `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs` | NEW — `StepListPane<'a>`: renders ordered step list with `✓/!/✗/…` status glyphs, selection highlight (accent bg when focused, subtle bg when unfocused), `step_list_pane()` convenience constructor |
| `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | NEW — `StepDetailPane<'a>`: Doctor step delegates to `DoctorView`, component steps render `ComponentCheck` rows; writes `last_known_visible_height` Cell each frame (annotated), render-time scroll clamp safety net, `step_detail_pane()` convenience constructor |
| `crates/fdemon-tui/src/widgets/install_wizard/doctor_view.rs` | NEW — `DoctorView<'a>`: renders `Option<&[DoctorLine]>` with per-marker coloring/glyph; `None` → unavailable placeholder; empty vec → no-output placeholder |
| `crates/fdemon-tui/src/widgets/mod.rs` | Added `pub mod install_wizard;` and `pub use install_wizard::InstallWizardPanel;` |
| `crates/fdemon-tui/Cargo.toml` | Added `fdemon-daemon` as a regular workspace dependency (needed for `DoctorLine`, `DoctorMarker`, `ComponentCheck`, `ComponentStatus` types in production code) |

### Notable Decisions/Tradeoffs

1. **fdemon-daemon as runtime dep in fdemon-tui**: `fdemon-tui` previously only had `fdemon-daemon` as a dev-dep. The install wizard widgets need `DoctorLine`, `DoctorMarker`, `ComponentCheck`, `ComponentStatus` in production rendering code. Adding it as a runtime dep is architecturally sound (`tui → daemon` is not a circular dependency). Cannot add re-exports to `fdemon-app/src/lib.rs` because scope boundary forbids touching fdemon-app.

2. **DoctorView accepts `&[DoctorLine]` not `&Vec<DoctorLine>`**: Changed from task spec to use the idiomatic slice reference, which allows `step_detail.rs` to pass a scrolled sub-slice without allocation. Clippy caught an `iter().cloned().collect()` issue that prompted this refactor.

3. **Render-hint write happens before early returns**: In `StepDetailPane`, `last_known_visible_height.set(visible_height)` is written before any early-return path so the handler always gets an accurate height regardless of which content branch is taken.

### Testing Performed

- `cargo fmt --all -- --check` - PASS
- `cargo check --workspace --all-targets` - PASS
- `cargo test --workspace` - PASS (2650+512+815+842+1393+80+... all pass, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` - PASS

### Risks/Limitations

1. **No render-branch wiring**: As specified, `InstallWizardPanel` is not yet rendered from `render/mod.rs` — that is task 05's scope. The widget is complete and tested in isolation but won't appear in the live TUI until task 05 connects it.

2. **Scroll only applies to Doctor view sub-slice**: The `detail_scroll` field clamps the doctor lines slice passed to `DoctorView`. For component steps the scroll is applied via a corrected offset over the components vec. Both paths include render-time clamp safety nets.
