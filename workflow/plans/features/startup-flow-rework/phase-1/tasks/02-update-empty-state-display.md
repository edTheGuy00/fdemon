## Task: Update Empty State Display Message

**Objective**: Change the centered message shown when no sessions exist from "Waiting for Flutter..." to a message instructing the user how to start a session.

**Depends on**: 01-modify-startup-logic

### Scope

- `src/tui/widgets/log_view/mod.rs`: Modify `render_empty()` function (lines 583-612)

### Details

The current `render_empty()` function displays:
- "Waiting for Flutter..."
- "Make sure you're in a Flutter project directory"

**Change to:**
- "Not Connected"
- "Press + to start a new session"

```rust
/// Render empty state with centered message
fn render_empty(&self, area: Rect, buf: &mut Buffer) {
    let block = Block::default()
        .title(self.title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    block.render(area, buf);

    // Center the instruction message
    let instruction_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Not Connected",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press + to start a new session",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    Paragraph::new(instruction_text)
        .alignment(ratatui::layout::Alignment::Center)
        .render(inner, buf);
}
```

### Acceptance Criteria

1. When no sessions exist, the log area shows "Not Connected" centered
2. Below that, shows "Press + to start a new session"
3. Styling uses DarkGray color similar to current implementation
4. "Not Connected" text is bold for emphasis
5. Works correctly in both normal and compact layouts

### Testing

Visual verification:
```bash
cargo run -- tests/fixtures/simple_app
# Should see centered "Not Connected" and instruction text
```

Unit tests in log_view module should still pass (if any exist for empty state).

### Notes

- Snapshot tests will fail after this change - they'll be updated in Phase 3
- The old message ("Waiting for Flutter...") will no longer appear anywhere
- Consider if we need different messages for different scenarios:
  - No sessions ever started: "Press + to start a new session"
  - All sessions closed: Same message (for simplicity)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/log_view/mod.rs` | Updated `render_empty()` function (lines 583-612) to display "Not Connected" with bold styling and "Press + to start a new session" instruction message |

### Notable Decisions/Tradeoffs

1. **Styling Consistency**: Used `DarkGray` color for both text elements to maintain consistency with existing UI. "Not Connected" uses `Modifier::BOLD` for emphasis while the instruction text remains normal weight.

2. **Message Simplicity**: Kept the message simple and action-oriented ("Press + to start a new session") rather than explaining the situation. This aligns with the user-initiated session model.

3. **Comment Update**: Changed the comment from "Center the waiting message" to "Center the instruction message" to reflect the new purpose of the empty state.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (compiled successfully in 0.97s)
- `cargo clippy -- -D warnings` - Passed (no warnings)
- `cargo test --lib log_view` - Passed (77 tests passed, 0 failed)

### Risks/Limitations

1. **Snapshot Test Failures**: As expected per task notes, 13 snapshot tests in `tui::render` module fail due to the changed text. These will be updated in Phase 3 as documented.

2. **Status Bar Test Failures**: 9 status bar tests fail, but these failures are due to Task 01's changes to `status_bar/mod.rs` (which added "Not Connected" logic to `state_indicator()`), not this task's changes. The tests need sessions to be created to properly test phase indicators.

3. **Visual-Only Verification**: The centered layout behavior is not covered by unit tests - it relies on ratatui's Paragraph alignment. Visual verification recommended via `cargo run -- tests/fixtures/simple_app`.
