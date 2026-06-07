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

**Status:** Not Started
**Branch:** <fill in>

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
