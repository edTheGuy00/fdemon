## Task: TUI polish — caret comments, named height const, placeholder copy, test coords, doc notes (S1 + N1 + N2 + N4 + N5 + N6)

**Objective**: Clean up the install-wizard TUI widgets per the review's should-fix/minor findings: correct
the self-contradictory caret/fill comments (S1), replace the magic `6` height literal with a named constant
(N1), soften the placeholder-leaf copy (N2), derive the render-test coordinates from constants instead of
literals (N4), document the `make_steps()` fixture's intentional `build_steps` bypass (N5), and add a
`step_caption` exhaustiveness note (N6). No behavior change beyond the user-facing copy (N2).

**Depends on**: None.

**Agent:** implementor

**Estimated Time**: 1.5–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs` — S1, N4, N5.
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` — N1.
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` — N2, N6.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/{state,types}.rs` — `WizardStepKind` (read only).

### Details

> Locate by symbol; line numbers will drift.

#### S1 — fix the contradictory caret/fill comments (`step_list.rs`)

In `render_step_row`, two comment blocks contradict the code:
- ≈242–244: claims the caret "is appended as plain unstyled text so it does not affect the row_style fill"
  — but the code styles it: `spans.push(Span::styled(c, row_style));` (≈264).
- ≈274–277: claims "The caret (and its preceding space) are NOT counted here" — but the fill **does** count
  them: `let suffix_len = caret.map(|c| 1 + c.chars().count() as u16).unwrap_or(0); let used = text_len + suffix_len;` (≈282–284).

The **code is correct** (the caret is styled with `row_style` and `suffix_len` reserves its width so no
stray highlighted cell appears at the row's right edge). Rewrite the comments to describe what the code
actually does — e.g.:

```rust
// Expand/collapse caret appended to the Platforms parent row. It is styled
// with `row_style` (so it participates in the selection highlight), and
// `suffix_len` below reserves its width so the background fill starts after it.
```

Remove the false "plain unstyled text" and "NOT counted here" clauses entirely. Keep one accurate
description near the `suffix_len`/`used` computation.

#### N1 — named constant for the dynamic-height clamp (`mod.rs`)

In `render_vertical_panes` (≈243–249) the clamp uses a bare literal:

```rust
.min(area.height.saturating_sub(6) as usize) // leave at least 6 rows for detail + sep
```

Per `docs/CODE_STANDARDS.md` Responsive Layout Principle 4 (named constants with derivation comments),
introduce a constant near the other layout constants (`VERTICAL_STEP_LIST_PADDING`, etc.):

```rust
/// Minimum rows reserved below the step-list pane in vertical layout for the
/// detail pane + separator, so an expanded step list never starves the detail view.
/// Derived from: detail-pane Constraint::Min(5) + 1 separator row = 6.
const MIN_DETAIL_RESERVE_ROWS: u16 = 6;
```

Use it in the `.saturating_sub(...)`. (Confirm the "5 + 1" derivation against the actual detail-pane
constraint in this function; adjust the comment to match what the layout really reserves.)

#### N2 — soften the placeholder-leaf copy (`step_detail.rs`)

In `render_action_hint` (≈250–276), the generic `else` arm renders `"  Available in a later phase"` for the
inert placeholder leaves (`PlatformWeb`/`PlatformIos`/`PlatformMacos`/`PlatformWindows`). Replace it with
user-facing, action-oriented copy that doesn't read like a developer TODO leak — e.g.
`"  Setup for this platform is coming soon — run flutter doctor to check it manually"` (keep it within the
pane width; match the existing two-space indent and muted style). Keep the `Doctor`/`Platforms` early-return
and the `PlatformAndroid`/`Prerequisites`-with-guided-commands early-return unchanged. Update any test that
asserts the exact old string.

#### N4 — derive render-test coordinates from constants (`step_list.rs`)

The render tests assert hardcoded buffer cells, e.g. `buf[(2, 2)]`, `buf[(4, 3)]`, `buf[(2, 4)]`, where
`2 = leading-space prefix`, `4 = 2 + INDENT_WIDTH` (leaf glyph x), `y = HEADER_HEIGHT + row_index`. Make the
arithmetic explicit using the existing constants (`HEADER_HEIGHT` is `pub(super)` at step_list.rs:72;
`INDENT_WIDTH = 2` at :67) instead of bare literals, so a future change to header height or indent width does
not silently invalidate the assertions. Either compute coords inline from the constants in each test, or add
a small test helper, e.g.:

```rust
// glyph cell for a row at the given visible index and indent depth
fn glyph_xy(row_index: u16, indent: u16) -> (u16, u16) {
    (LEADING_PREFIX_COLS + INDENT_WIDTH * indent, HEADER_HEIGHT + row_index)
}
```

where `LEADING_PREFIX_COLS` is the existing 2-space prefix (introduce a named const for the `2` baked into
`" ".repeat(2 + …)` at ≈240 if one doesn't exist). Tests affected (from research):
`test_selected_step_highlighted`, `test_unfocused_selected_uses_subtle_highlight`,
`step_list_shows_failed_indicator_after_failed_execution`,
`step_list_failed_badge_does_not_affect_other_steps`, `step_list_no_failed_badge_when_execution_is_none`,
`run_failed_badge_is_bold_plain_missing_is_not`, `leaf_row_glyph_is_indented_relative_to_top_level`.
Keep the assertions equivalent (same cells), just constant-derived.

#### N5 — document the `make_steps()` fixture (`step_list.rs`)

`make_steps()` (≈362–400) hand-rolls a flat `Vec<WizardStep>` and intentionally does **not** call
`build_steps`. Add a one-line note in the helper body/doc making the intent explicit, e.g.:
`// Deliberately hand-rolled (not build_steps) so render-test coordinates stay fixed and independent of the
collapsed/expanded projection.`

#### N6 — `step_caption` exhaustiveness note (`step_detail.rs`)

On the `_ => None` arm of `step_caption` (≈90–100, currently only `PlatformAndroid` and `Prerequisites`
have captions), add:
`// Phase 2: only PlatformAndroid and Prerequisites have captions. A new leaf caption also needs a
// corresponding executor/handler arm — keep this in sync.`

### Acceptance Criteria

1. No comment in `render_step_row` claims the caret is unstyled or uncounted; the remaining comment matches
   the actual `row_style` styling + `suffix_len` fill. No behavior change (selected Platforms parent row
   still has no stray highlight cell).
2. The `6` height literal is replaced by a named constant with a derivation comment; vertical-layout
   rendering is unchanged.
3. Placeholder-leaf detail hint shows the softened, user-facing copy; no remaining "Available in a later
   phase" string (or its test).
4. Render-test glyph/badge coordinates are derived from `HEADER_HEIGHT`/`INDENT_WIDTH`/prefix constants, not
   bare literals; all the listed tests still pass and assert the same cells.
5. `make_steps()` carries a note explaining the deliberate `build_steps` bypass; `step_caption`'s `_ => None`
   arm carries the exhaustiveness note.
6. `cargo test --workspace --lib` green; `cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings` clean.

### Testing

```bash
cargo test -p fdemon-tui --lib install_wizard
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
```

### Notes

- N2 is the only user-visible change; everything else is comments/constants/test-refactor. If N2's new copy
  changes a snapshot/string test, update that test.
- Disjoint files from Tasks 01 and 02 → safe to run in a parallel worktree.
- Keep glyphs/caret characters (`CARET_EXPANDED`/`CARET_COLLAPSED`) and styling consistent with the existing
  status-icon palette.

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
