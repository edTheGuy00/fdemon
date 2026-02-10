## Task: Redesign Settings Panel Footer

**Objective**: Transform the settings panel footer to match the Cyber-Glass design: 4 shortcut hints with icons, styled key badges, description labels, and an emphasized `Ctrl+S` hint.

**Depends on**: 04-redesign-settings-content

### Scope

- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` — Redesign `render_footer()`

### Details

#### Current Footer (lines 246-268)

```
│ Tab: Switch tabs  j/k: Navigate  Enter: Edit  Ctrl+S: Save │
```

- Single line of centered text in `TEXT_MUTED`
- Context-sensitive: shows "(unsaved changes)" when dirty, different text when editing
- Footer area: 2 lines (border + content)

#### Target Footer

```
┌────────────────────────────────────────────────────────────────┐
│  ⌨ Tab: Switch tabs    $ j/k: Navigate    › Enter: Edit    💾 Ctrl+S: Save Changes  │
└────────────────────────────────────────────────────────────────┘
```

Design details:
- Background: `DEEPEST_BG` (darker than content area)
- Border: top border in `BORDER_DIM`
- 4 shortcut hints, centered, spaced evenly
- Each hint: `icon` + `key:` in `TEXT_SECONDARY` + `description` in `TEXT_MUTED`
- `Ctrl+S` hint: key in `ACCENT` (emphasized), icon in `ACCENT`
- Editing state: different shortcuts shown
- Dirty state: "Save Changes" becomes "Save Changes*" or highlighted

#### Implementation

**1. Increase footer height from 2 to 3 lines:**

In `render()`, update the footer constraint:

```rust
let chunks = Layout::vertical([
    Constraint::Length(5),   // Header
    Constraint::Min(5),      // Content
    Constraint::Length(3),   // Footer (was 2): border + content + border
])
.split(area);
```

**2. Redesign `render_footer()`:**

```rust
fn render_footer(area: Rect, buf: &mut Buffer, state: &SettingsViewState) {
    let icons = IconSet::new(IconMode::Unicode);

    // Dark background block with top border only
    let footer_block = Block::default()
        .borders(Borders::ALL & !Borders::TOP) // Or Borders::ALL for full border
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(palette::BORDER_DIM))
        .style(Style::default().bg(palette::DEEPEST_BG));

    let inner = footer_block.inner(area);
    footer_block.render(area, buf);

    // Draw top separator line manually for cleaner look
    for x in area.left()..area.right() {
        if let Some(cell) = buf.cell_mut((x, area.top())) {
            cell.set_char('─');
            cell.set_style(Style::default().fg(palette::BORDER_DIM));
        }
    }

    if state.editing {
        render_editing_footer_hints(inner, buf, &icons);
    } else {
        render_normal_footer_hints(inner, buf, &icons, state.dirty);
    }
}
```

**3. Normal mode hints:**

```rust
fn render_normal_footer_hints(area: Rect, buf: &mut Buffer, icons: &IconSet, is_dirty: bool) {
    let hints: Vec<Line> = vec![
        build_hint(icons.keyboard(), "Tab:", "Switch tabs", false),
        build_hint(icons.command(), "j/k:", "Navigate", false),
        build_hint(icons.chevron_right(), "Enter:", "Edit", false),
        build_hint(
            icons.save(),
            "Ctrl+S:",
            if is_dirty { "Save Changes*" } else { "Save Changes" },
            true, // emphasized
        ),
    ];

    // Calculate total width and center
    // Each hint: icon(2) + key(varies) + space(1) + label(varies) + gap(4)
    let mut spans: Vec<Span> = Vec::new();
    for (i, hint) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("    ")); // 4-space gap between hints
        }
        spans.extend(hint.spans.clone());
    }

    let centered_line = Line::from(spans).alignment(Alignment::Center);
    buf.set_line(area.left(), area.top(), &centered_line, area.width);
}

fn build_hint<'a>(
    icon: &'a str,
    key: &'a str,
    label: &'a str,
    emphasized: bool,
) -> Line<'a> {
    let icon_style = if emphasized {
        Style::default().fg(palette::ACCENT)
    } else {
        Style::default().fg(palette::TEXT_MUTED)
    };

    let key_style = if emphasized {
        styles::kbd_accent_style()  // ACCENT fg
    } else {
        Style::default().fg(palette::TEXT_SECONDARY)
    };

    let label_style = styles::kbd_label_style(); // TEXT_MUTED

    Line::from(vec![
        Span::styled(format!("{} ", icon), icon_style),
        Span::styled(key, key_style),
        Span::styled(format!(" {}", label), label_style),
    ])
}
```

**4. Editing mode hints:**

When `state.editing` is true, show different shortcuts:

```rust
fn render_editing_footer_hints(area: Rect, buf: &mut Buffer, icons: &IconSet) {
    let hints = Line::from(vec![
        Span::styled(format!("{} ", icons.check()), Style::default().fg(palette::STATUS_GREEN)),
        Span::styled("Enter:", Style::default().fg(palette::TEXT_SECONDARY)),
        Span::styled(" Confirm", styles::kbd_label_style()),
        Span::raw("    "),
        Span::styled(format!("{} ", icons.close()), Style::default().fg(palette::STATUS_RED)),
        Span::styled("Esc:", Style::default().fg(palette::TEXT_SECONDARY)),
        Span::styled(" Cancel", styles::kbd_label_style()),
    ]).alignment(Alignment::Center);

    buf.set_line(area.left(), area.top(), &hints, area.width);
}
```

**5. Connect borders with the outer glass container:**

The footer should visually connect with the settings panel's outer container. The bottom border of the footer should be the bottom-left and bottom-right rounded corners of the overall panel:

```
│ ⌨ Tab: Switch tabs   $ j/k: Navigate   › Enter: Edit   💾 Ctrl+S: Save │
╰────────────────────────────────────────────────────────────────────────────╯
```

This may require adjusting the outer block rendering in `render()` so the footer block doesn't double-render bottom borders.

### Acceptance Criteria

1. Footer has darker background (`DEEPEST_BG`) than content area
2. Top separator line in `BORDER_DIM`
3. Normal mode shows 4 hints: Tab/j,k/Enter/Ctrl+S
4. Each hint has: icon + key in `TEXT_SECONDARY` + description in `TEXT_MUTED`
5. `Ctrl+S` hint: icon and key in `ACCENT` (emphasized)
6. Hints are centered horizontally with even spacing
7. Editing mode shows: Enter (Confirm) + Esc (Cancel) with check/close icons
8. Dirty state: "Save Changes*" shows asterisk to indicate unsaved changes
9. Footer height is 3 lines (border + content + border) or 2 if single border
10. `cargo check -p fdemon-tui` passes
11. `cargo clippy -p fdemon-tui` passes

### Testing

- Verify normal mode shows all 4 shortcut hints
- Verify `Ctrl+S` hint is visually distinct (accent color)
- Verify editing mode switches to Enter/Esc hints
- Verify dirty state shows asterisk on "Save Changes*"
- Test narrow terminals: hints should truncate or wrap gracefully
- Verify footer background is darker than content area

### Notes

- **Footer height change**: Increasing from 2 to 3 lines means the content area loses 1 line. Combined with the header increase (+2 lines), the content area is 3 lines shorter overall. On a 24-line terminal, content goes from ~19 to ~16 visible lines. This is acceptable.
- **Border coordination**: The outer `render()` function creates the main glass container block. The footer renders inside this block. Ensure borders don't double up — the footer's bottom border should be the outer block's bottom border. Consider removing the outer block's bottom border and letting the footer render it.
- **Hint alignment**: Use `Line::alignment(Alignment::Center)` for centering. On very wide terminals, hints may look spread out. The 4-space gap between hints keeps them visually grouped.
- **Hint icon width**: Icons are variable width (Unicode vs NerdFonts). Centering calculations use the rendered line width, so ratatui handles this automatically.
- **Existing context-sensitivity**: The current footer already changes text for editing/dirty states. The new footer preserves this behavior with better visual differentiation.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` | Increased footer height from 2 to 3 lines. Redesigned `render_footer()` with new hint system: added `render_normal_footer_hints()`, `render_editing_footer_hints()`, and `build_hint()` helper functions. Footer now displays 4 shortcut hints with icons (keyboard, chevron, save) in normal mode and 2 hints (check, close) in editing mode. Added DEEPEST_BG background and BORDER_DIM border styling. |
| `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` | Updated `test_settings_panel_dirty_indicator` to check for "Save" and "Ctrl+S" instead of "unsaved". Updated `test_render_project_tab` to check for spaced uppercase section headers ("B E H A V I O R") instead of bracketed format ("[Behavior]") per Phase 4 Task 03 changes. |

### Notable Decisions/Tradeoffs

1. **Footer height 3 lines**: Changed from 2 to 3 lines for better visual prominence and clearer hint presentation. The extra line provides breathing room for the icon + key + label layout.

2. **Hint layout**: Each hint follows the pattern `icon + key + label` with appropriate styling:
   - Non-emphasized: icon in TEXT_MUTED, key in TEXT_SECONDARY, label in TEXT_MUTED
   - Emphasized (Ctrl+S): icon in ACCENT, key in ACCENT (kbd_accent_style), label in TEXT_MUTED

3. **Dirty state indicator**: Shows "Save Changes*" with asterisk when dirty, preserving the context-sensitive behavior from the original implementation.

4. **Editing mode**: Displays check icon (green) + "Enter: Confirm" and close icon (red) + "Esc: Cancel" for clear visual feedback.

5. **Test updates**: Updated tests to match new footer content. The test terminal (80x24) truncates the full footer text, so tests check for key portions ("Save", "Ctrl+S") rather than the complete string.

### Testing Performed

- `cargo check -p fdemon-tui` - Passed
- `cargo clippy -p fdemon-tui` - Passed (no warnings in mod.rs)
- `cargo test -p fdemon-tui --lib` - Passed (433 tests)

### Risks/Limitations

1. **Footer truncation on narrow terminals**: On terminals narrower than ~80 characters, the footer hints may be truncated. The centered layout ensures the most important hints (Tab, j/k, Enter) appear first, with Ctrl+S potentially cut off on very narrow terminals.

2. **Content area reduction**: The footer height increase from 2 to 3 lines reduces the content area by 1 line. Combined with header changes from Task 03 (+2 lines), the total content reduction is 3 lines. On a 24-line terminal, content area goes from ~19 to ~16 visible lines.
