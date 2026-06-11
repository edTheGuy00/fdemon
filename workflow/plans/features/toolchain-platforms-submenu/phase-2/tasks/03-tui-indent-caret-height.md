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

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs` | Added `platforms_expanded: bool` to `StepListPane`; updated `new()` and `step_list_pane()` with new param; added `CARET_EXPANDED`/`CARET_COLLAPSED`/`INDENT_WIDTH` constants; promoted `HEADER_HEIGHT` to `pub(super)`; updated `render_step_row` to indent leaves by `step.indent * INDENT_WIDTH` and append caret on `WizardStepKind::Platforms`; updated fill-width math to account for caret suffix; updated all `StepListPane::new` test calls (+`false` arg); fixed `test_selected_step_highlighted` to use top-level step (index 0); fixed `step_list_failed_badge_does_not_affect_other_steps` PlatformAndroid glyph x from 2 to 4; added `make_steps_with_platforms_parent`, `make_steps_with_expanded_platforms` fixtures; added 4 new tests: `platforms_parent_shows_collapsed_caret`, `platforms_parent_shows_expanded_caret`, `leaf_row_glyph_is_indented_relative_to_top_level`, `step_list_height_grows_when_expanded` |
| `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` | Replaced `VERTICAL_STEP_LIST_HEIGHT = 9` with `VERTICAL_STEP_LIST_PADDING = 2` constant; added `use step_list::HEADER_HEIGHT`; updated `render_vertical_panes` to compute step-list height dynamically from `steps.len()`; passed `state.platforms_expanded` to `step_list_pane` in both horizontal and vertical layout paths; updated `render_footer` to append `· [Enter] expand/collapse` when selected step is `WizardStepKind::Platforms` |

### Notable Decisions/Tradeoffs

1. **`HEADER_HEIGHT` promoted to `pub(super)` instead of duplicating the constant**: The vertical panes height calculation needs `HEADER_HEIGHT` from `step_list.rs`. Promoting it to `pub(super)` is the correct layering — `mod.rs` is the direct parent and already imports from `step_list`. No public API surface change.

2. **Caret NOT included in selection-highlight fill width math in the obvious sense**: The caret is rendered via `Span::styled(c, row_style)`, so ratatui applies `row_style` to it automatically. The fill only needs to cover the gap between the end of the title and the right edge. The `suffix_len` variable correctly accounts for the caret + preceding space in the `used` total so the fill starts at the right offset.

3. **Test `test_selected_step_highlighted` changed to index 0 (Prerequisites)**: The task specified "Prefer building fixture collapsed where test only needs a top-level step." The original test selected index 1 (PlatformAndroid, indent=1), which shifts the glyph to x=4. Switching to index 0 keeps `buf[(2, 2)]` stable and makes the test invariant to indent changes.

4. **Dynamic height clamped to leave at least 6 rows for the detail pane**: `area.height.saturating_sub(6)` ensures the detail pane always gets a non-trivial allocation even if step count is large. A minimum of 6 rows (1 separator + 5 for Constraint::Min(5)) was chosen to match the existing `Constraint::Min(5)` floor.

### Testing Performed

- `cargo test -p fdemon-tui --lib install_wizard` — Passed (128 tests)
- `cargo test --workspace --lib` — Passed (1487 tests)
- `cargo fmt --all -- --check` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Caret columns not counted in terminal-width overflow guards**: The caret adds 2 display columns (`" ▸"` or `" ▾"`) to the Platforms row. On very narrow terminals (< ~20 columns) the step title + caret may wrap or clip. This is acceptable given `MIN_RENDER_WIDTH = 40` and the existing `Paragraph` wrapping behaviour; no stray highlighted cells are produced because the fill math accounts for the suffix.

2. **`VERTICAL_STEP_LIST_HEIGHT` constant removed**: Any code outside this module referencing that constant by name would break. A search confirmed it was only used internally in `render_vertical_panes`; no external callers.
