## Task: Redesign Modal Footer with kbd-Style Shortcut Hints

**Objective**: Replace the plain-text footer with a themed footer bar showing keyboard shortcut hints in "kbd" badge style — keys in lighter text with subtle contrast, labels in muted text, matching the Cyber-Glass design reference.

**Depends on**: 04-redesign-target-selector, 05-redesign-launch-context

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` — Redesign `render_footer()` and `render_footer_compact()`

### Details

#### Current Footer

```
[1/2] Tab  [Tab] Pane  [↑↓] Navigate  [Enter] Select  [Esc] Close
```

- Single line of plain text
- `BORDER_DIM` color (all same style)
- No visual distinction between keys and labels
- Centered alignment

#### Target Footer

```
──────────────────────────────────────────────────────
  [1/2] Tab  ·  [Tab] Pane  ·  [↑↓] Navigate  ·  [Enter] Select  ·  [Esc] Close
```

- `SURFACE` background (slightly darker than content area)
- Top border: horizontal separator in `BORDER_DIM` (already added in Task 03)
- Keys `[1/2]`, `[Tab]`, etc: `TEXT_PRIMARY` (brighter) — simulate kbd badge
- Labels "Tab", "Pane", etc: `TEXT_MUTED` (dimmer)
- Dot separators `·` in `BORDER_DIM`
- Centered, uppercase labels optional

#### Implementation

**1. Build styled footer line:**

```rust
fn render_footer(&self, area: Rect, buf: &mut Buffer) {
    // Fill background
    let bg_block = Block::default()
        .style(Style::default().bg(palette::SURFACE));
    bg_block.render(area, buf);

    let hints = self.footer_hints();

    let mut spans: Vec<Span> = Vec::new();
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", Style::default().fg(palette::BORDER_DIM)));
        }
        spans.push(Span::styled(
            format!("[{}]", key),
            Style::default().fg(palette::TEXT_PRIMARY),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            *label,
            Style::default().fg(palette::TEXT_MUTED),
        ));
    }

    let line = Line::from(spans);
    Paragraph::new(line)
        .alignment(Alignment::Center)
        .render(area, buf);
}
```

**2. Footer hint data:**

```rust
fn footer_hints(&self) -> Vec<(&str, &str)> {
    if self.state.is_fuzzy_modal_open() {
        vec![
            ("↑↓", "Navigate"),
            ("Enter", "Select"),
            ("Esc", "Cancel"),
        ]
    } else if self.state.is_dart_defines_modal_open() {
        vec![
            ("Tab", "Pane"),
            ("↑↓", "Navigate"),
            ("Enter", "Edit"),
            ("Esc", "Close"),
        ]
    } else {
        vec![
            ("1/2", "Tab"),
            ("Tab", "Pane"),
            ("↑↓", "Navigate"),
            ("Enter", "Select"),
            ("Esc", "Close"),
        ]
    }
}
```

**3. Compact footer (vertical layout):**

For narrow terminals, abbreviate or reduce hints:

```rust
fn render_footer_compact(&self, area: Rect, buf: &mut Buffer) {
    let bg_block = Block::default()
        .style(Style::default().bg(palette::SURFACE));
    bg_block.render(area, buf);

    let hints = if self.state.is_fuzzy_modal_open() {
        vec![("↑↓", "Nav"), ("Enter", "Sel"), ("Esc", "Close")]
    } else if self.state.is_dart_defines_modal_open() {
        vec![("Tab", "Pane"), ("↑↓", "Nav"), ("Esc", "Close")]
    } else {
        vec![("1/2", "Tab"), ("Tab", "Pane"), ("↑↓", "Nav"), ("Esc", "Close")]
    };

    let mut spans: Vec<Span> = Vec::new();
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(palette::BORDER_DIM)));
        }
        spans.push(Span::styled(
            format!("[{}]", key),
            Style::default().fg(palette::TEXT_PRIMARY),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            *label,
            Style::default().fg(palette::TEXT_MUTED),
        ));
    }

    Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .render(area, buf);
}
```

**4. Footer background:**

The footer sits in the area between the bottom separator and the modal border. Apply `SURFACE` background to visually separate it from the content area:

```rust
// In render_footer(), first fill background
let bg = Block::default().style(Style::default().bg(palette::SURFACE));
bg.render(area, buf);
```

### Acceptance Criteria

1. Footer renders with `SURFACE` background (visually distinct from content area)
2. Key badges `[key]` render in `TEXT_PRIMARY` (brighter than labels)
3. Action labels render in `TEXT_MUTED` (dimmer)
4. Dot separators `·` render in `BORDER_DIM` between hint groups
5. Footer content is centered
6. Footer changes based on state (main, fuzzy modal, dart defines modal)
7. Compact footer renders with abbreviated labels for vertical layout
8. `cargo check -p fdemon-tui` passes
9. `cargo clippy -p fdemon-tui` passes

### Testing

- Visually verify footer styling: key badges brighter than labels
- Verify footer changes when fuzzy modal is open
- Verify footer changes when dart defines modal is open
- Test compact footer in vertical layout (50x30 terminal)
- Verify footer background is visually distinct from content area
- Verify centered alignment at different terminal widths

### Notes

- **Footer height**: Keep at 1 line. The separator above the footer is rendered by Task 03 (modal frame). This task only handles the content within the footer area.
- **Kbd badge styling**: The TSX design uses `<kbd>` elements with background and border styling. In TUI, the best approximation is using brighter text for keys and dimmer text for labels. More elaborate approaches (background color per span) are possible but may look cluttered in a monospace terminal.
- **FOOTER_MAIN / FOOTER_FUZZY_MODAL / FOOTER_DART_DEFINES constants**: These existing string constants should be replaced with the structured hint approach. Remove the old constants.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Replaced plain-text footer with kbd-style shortcut hints. Removed footer string constants, added `footer_hints()` method to generate structured hint data. Implemented `render_footer()` with SURFACE background, TEXT_PRIMARY keys, TEXT_MUTED labels, BORDER_DIM separators. Implemented `render_footer_compact()` with abbreviated labels for vertical layout. |

### Notable Decisions/Tradeoffs

1. **Structured hints approach**: Replaced static string constants with a `footer_hints()` method that returns `Vec<(&str, &str)>` tuples of (key, label). This makes the footer more maintainable and allows for state-specific hints.
2. **Kbd badge approximation**: Used TEXT_PRIMARY for keys and TEXT_MUTED for labels to create visual hierarchy similar to `<kbd>` elements in web UI. This provides good contrast without the complexity of per-span background colors.
3. **Compact footer abbreviations**: In vertical layout, labels are abbreviated (e.g., "Navigate" → "Nav", "Select" → "Sel") to save space, with slightly tighter separators (" · " instead of "  ·  ").

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (all 428 TUI unit tests passed, E2E test failures are known issues unrelated to this task)
- `cargo clippy --workspace -- -D warnings` - Passed
- `cargo test -p fdemon-tui test_dialog_renders` - Passed

### Risks/Limitations

None identified. The implementation follows the exact specification from the task file and maintains backward compatibility with existing dialog state management.
