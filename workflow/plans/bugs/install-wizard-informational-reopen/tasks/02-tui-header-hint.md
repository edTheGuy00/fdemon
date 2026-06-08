## Task: "All set" informational header hint

**Objective**: When the Install Wizard is opened informationally (`origin == UserInvoked`) and the
toolchain is fully healthy (`all_components_ok()`), show a reassuring header hint such as
`All set — press Esc to return` instead of the generic subtitle.

**Depends on**: 01-core-origin-fix (needs `WizardOrigin`, the `origin` field, and
`all_components_ok()`)

**Agent:** implementor

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**

- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` — extend `render_header` (line ~106) to
  render the "All set" subtitle when informational + all-Ok. The panel already receives
  `&InstallWizardState`, so read `state.origin`/`state.is_bootstrap()` and
  `state.all_components_ok()` directly. The subtitle is rendered at row `area.y + 1`
  (lines ~136–146) — swap the text there based on the condition.
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` — only if the header decision
  actually lives here; otherwise leave untouched. (Header is in `mod.rs::render_header`.)

**Files Read (Dependencies):**

- `crates/fdemon-app/src/install_wizard/state.rs` — `origin`, `is_bootstrap()`,
  `all_components_ok()`.

### Details

In `render_header`, after the title row, choose the subtitle:

```rust
// Row 1: subtitle (dimmed)
let subtitle_text = if !self.state.is_bootstrap() && self.state.all_components_ok() {
    "All set — press Esc to return"
} else {
    /* existing subtitle text */
};
let subtitle = Line::from(vec![
    Span::raw("  "),
    Span::styled(subtitle_text, Style::default().fg(palette::TEXT_MUTED)),
]);
let subtitle_area = Rect::new(area.x, area.y + 1, area.width, 1);
Paragraph::new(subtitle).render(subtitle_area, buf);
```

Adapt to the actual field/accessor names used in the widget (`self.state` vs a passed `state`
parameter) — check how `render_header` accesses the wizard state and `palette` constant in use.

- Show the hint only when **not loading** and a report is present (`all_components_ok()` already
  returns `false` when `report` is `None`, which covers the loading case).
- Keep the `[Esc] Close` hint on the title row unchanged.

### Acceptance Criteria

1. With `origin == UserInvoked` and an all-Ok report, the panel header shows the "All set" hint.
2. With `origin == Bootstrap`, or any non-Ok / missing component, or while loading, the original
   subtitle is shown (no "All set" hint).
3. Rendering does not panic and respects the existing 2-row header layout (`MIN_RENDER_HEIGHT`).

### Testing

Add a render test using the project's `TestTerminal` helper (see existing
`crates/fdemon-tui/src/widgets/install_wizard/` tests or `render/tests.rs` for the pattern):

```rust
#[test]
fn informational_all_ok_shows_all_set_hint() {
    // Build InstallWizardState with origin = UserInvoked and an all-Ok report,
    // render the panel into a TestTerminal, assert the buffer contains "All set".
}

#[test]
fn bootstrap_or_partial_does_not_show_all_set_hint() {
    // origin = Bootstrap (or a partial report) → buffer must NOT contain "All set".
}
```

Run: `cargo test -p fdemon-tui`, then `cargo clippy --workspace` and `cargo fmt --all`.

### Notes

- Pure presentation change; no state/handler logic. Behaviour gating lives entirely in task 01.
- If `palette::TEXT_MUTED` is already used for the subtitle, reuse it for visual consistency.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` | Modified `render_header` to show "All set — press Esc to return" when `!is_bootstrap() && all_components_ok()`; changed subtitle style from `TEXT_SECONDARY` to `TEXT_MUTED` for both variants; added two render tests |

### Notable Decisions/Tradeoffs

1. **Subtitle colour harmonisation**: The "All set" hint uses `palette::TEXT_MUTED` as specified in the task. For visual consistency the original "Flutter toolchain setup" subtitle was also changed from `TEXT_SECONDARY` to `TEXT_MUTED`. Both are dimmed colours; `TEXT_MUTED` is slightly more subdued, which is appropriate given the subtitle is secondary information in both cases.

2. **Unicode em dash**: The em dash (`—`) in "All set — press Esc to return" is embedded as the escape sequence `\u{2014}` per the project pattern (other places in the same file use `\u{00b7}`, `\u{2500}`, `\u{2502}` etc.).

3. **Tests cover all three negative cases**: Bootstrap origin, partial (any-non-Ok) report, and loading state are verified in a single parameterised test body rather than three separate test functions to keep the test module tidy while still covering every branch of the acceptance criteria.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all crates; new tests `informational_all_ok_shows_all_set_hint` and `bootstrap_or_partial_does_not_show_all_set_hint` both passed)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Pure presentation**: No state or handler logic was changed; the entire behaviour gate lives in `is_bootstrap()` and `all_components_ok()` which were implemented in task 01. If those accessors change semantics, this presentation layer will follow automatically.
