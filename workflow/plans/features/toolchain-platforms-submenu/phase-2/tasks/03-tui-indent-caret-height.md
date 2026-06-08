## Task: TUI — indent leaf rows, expand/collapse caret, dynamic step-list height, footer hint

**Objective**: Render the Platforms submenu: indent leaf rows, draw an expand/collapse caret on the
`Platforms` parent, make the step-list pane height adapt to the (dynamic) visible-step count, and add an
expand/collapse hint to the footer when the parent is selected.

**Depends on**: Task 01 (`WizardStep.indent`, `InstallWizardState.platforms_expanded`,
`WizardStepKind::Platforms`/`is_platform_leaf()` exist). Reads `platforms_expanded` (Task 02 toggles it,
but this task only renders the flag — no dependency on 02 to compile).

**Agent:** implementor

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs` — indent + caret in `render_step_row`;
  `StepListPane` gains a `platforms_expanded` field; render tests (x/y-coordinate fixups).
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` — dynamic step-list height; footer hint;
  thread `platforms_expanded` into `step_list_pane`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/{types,state}.rs` — `WizardStep.indent`, `platforms_expanded`,
  `WizardStepKind`.

### Details

> The `make_steps()` test helper + `PlatformAndroid` rename in `step_list.rs` were done in Task 01; this
> task edits the render code and render tests in the same file. Locate by symbol; lines will drift.

#### 1. Indent leaf rows (`render_step_row`)

`render_step_row` currently renders `"  " + glyph + " " + title`. Indent by `step.indent`:
- Add `indent * 2` extra leading spaces before the glyph (leaf rows → 2 extra spaces, so `"    " + glyph
  + " " + title`).
- Ensure the **background-fill / padding** after the title accounts for the added prefix width so no stray
  highlighted cell is left at the row end (the existing fill computes from `text_len`).

#### 2. Expand/collapse caret on the parent

When `step.kind == WizardStepKind::Platforms`, append a caret reflecting `platforms_expanded`:
`▾` when expanded, `▸` when collapsed (e.g. after the title, or as a prefix glyph). Pass the flag in:
- Add `platforms_expanded: bool` to the `StepListPane` struct, its `new()` (or builder), and the
  `step_list_pane(...)` convenience constructor.
- In `mod.rs`, pass `state.install_wizard_state.platforms_expanded` when constructing the pane (in both the
  vertical and horizontal layout paths if both build the pane).

Keep the caret out of the selection-highlight width math (account for its 1–2 columns in the fill).

#### 3. Dynamic step-list height (`mod.rs`)

`VERTICAL_STEP_LIST_HEIGHT = 9` is hardcoded (`header(2) + 5 rows + 2 padding`). With a variable visible
count (5 collapsed; up to ~9 expanded on macOS), compute it from `state.install_wizard_state.steps.len()`:

```rust
// header rows + one row per visible step (+ existing padding, if any)
let step_list_height = (HEADER_HEIGHT as usize + state.install_wizard_state.steps.len()) as u16;
```

Use this for the `Constraint::Length(...)` of the step-list pane (clamp to the available area). Remove or
repurpose the `VERTICAL_STEP_LIST_HEIGHT` constant. No test asserts its literal value.

#### 4. Footer hint (`mod.rs` `render_footer`)

The footer is `"[Tab] switch · [j/k] move · [r] re-run · [Esc] close"`. When the selected step is the
`Platforms` parent, append `· [Enter] expand/collapse`. Make `render_footer` context-aware via
`state.install_wizard_state.selected_step().map(|s| s.kind) == Some(WizardStepKind::Platforms)`.

#### 5. Tests

- Update the run-failed-badge / glyph tests that assert fixed `buf[(x, y)]` cells: with the `Platforms`
  parent at row 1 and (when expanded) indented leaves, the glyph **x-coordinate shifts for leaf rows** and
  **y-coordinates shift** for steps below an expanded parent. Update the affected coordinate assertions
  (`step_list_shows_failed_indicator_after_failed_execution`,
  `step_list_failed_badge_does_not_affect_other_steps`, `run_failed_badge_is_bold_plain_missing_is_not`,
  `test_selected_step_highlighted`). Prefer building the fixture **collapsed** where the test only needs a
  top-level step, to keep coordinates stable.
- Add render tests: parent row shows `▸` collapsed / `▾` expanded; a leaf row is indented (glyph x greater
  than a top-level row's); the step-list height grows when expanded.

```bash
cargo test -p fdemon-tui --lib install_wizard
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

### Acceptance Criteria

1. Leaf rows (`indent == 1`) render indented relative to top-level rows; no stray highlight cells.
2. The `Platforms` parent shows `▾` when `platforms_expanded`, `▸` otherwise.
3. The step-list pane height adapts to the visible step count (collapsed vs expanded) without clipping.
4. The footer shows the expand/collapse hint only when the parent is selected.
5. Existing render tests updated for the new coordinates; new caret/indent/height tests pass.
6. `cargo test --workspace --lib` green; fmt + clippy clean.

### Notes

- This task only **renders** `platforms_expanded` / `indent`; it does not mutate them (Task 02 owns toggling).
  It compiles and renders correctly even if Task 02 is not yet merged (flag stays `false` → collapsed view).
- Disjoint files from Task 02 → safe to run in a parallel worktree after Task 01.
- Keep the glyphs/styling consistent with the existing status-icon palette in `step_list.rs`.

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
