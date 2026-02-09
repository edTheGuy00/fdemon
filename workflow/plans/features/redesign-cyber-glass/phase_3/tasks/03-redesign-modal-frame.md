## Task: Redesign Modal Frame and Header

**Objective**: Transform the dialog's outer frame from a simple bordered box with "New Session" title-on-border into a Cyber-Glass glass container with a distinct header area showing title, subtitle, and close hint — separated from the content area.

**Depends on**: 01-migrate-palette-to-rgb

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` — Redesign dialog block, header, and internal layout

### Details

#### Current Frame

```
╭──────────── New Session ─────────────╮
│ [Target Selector] │ [Launch Context] │
│                   │                  │
│                   │                  │
╰──────────────────────────────────────╯
 [1/2] Tab  [Tab] Pane  [↑↓] Navigate...
```

- Title is on the border (`.title(" New Session ")`)
- Background: `POPUP_BG` (currently DarkGray, will be Rgb(28,33,43))
- No header area, no subtitle
- Footer is 1-line text below the content

#### Target Frame

```
╭──────────────────────────────────────╮
│                                      │
│  New Session                  [Esc]  │
│  Configure deployment target and     │
│  runtime flags.                      │
│                                      │
│──────────────────────────────────────│
│ [Target Selector] │ [Launch Context] │
│                   │                  │
│                   │                  │
│──────────────────────────────────────│
│  [1/2] Tab · [Tab] Pane · [Enter]   │
╰──────────────────────────────────────╯
```

- `POPUP_BG` background (Rgb(28,33,43))
- `BorderType::Rounded`, `BORDER_DIM` border color
- Header area (3-4 lines) with title + subtitle + close hint
- Horizontal separator between header and content
- Horizontal separator between content and footer
- Footer area (1-2 lines) with themed shortcut hints

#### Implementation

**1. Remove title from border:**

```rust
// Before
let block = Block::default()
    .title(" New Session ")
    .title_alignment(Alignment::Center)
    .borders(Borders::ALL)
    .border_set(symbols::border::ROUNDED)
    .style(Style::default().bg(palette::POPUP_BG));

// After
let block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(styles::border_inactive())
    .style(Style::default().bg(palette::POPUP_BG));
```

**2. New internal layout:**

Split the inner area into header, separator, content, separator, footer:

```rust
let inner = block.inner(dialog_area);
let chunks = Layout::vertical([
    Constraint::Length(3),  // Header (title + subtitle)
    Constraint::Length(1),  // Separator
    Constraint::Min(10),   // Content (panes)
    Constraint::Length(1),  // Separator
    Constraint::Length(1),  // Footer
])
.split(inner);
```

**3. Render header area:**

```rust
fn render_header(&self, area: Rect, buf: &mut Buffer) {
    // Row 1: "New Session" (left) + "[Esc] Close" (right)
    let title_line = Line::from(vec![
        Span::raw("  "),
        Span::styled("New Session", Style::default()
            .fg(palette::TEXT_BRIGHT)
            .add_modifier(Modifier::BOLD)),
    ]);

    let close_hint = Line::from(vec![
        Span::styled("[Esc]", Style::default().fg(palette::TEXT_MUTED)),
        Span::raw(" "),
        Span::styled("Close", Style::default().fg(palette::TEXT_MUTED)),
        Span::raw("  "),
    ]);

    // Split area for title (left) and close hint (right)
    let title_area = Rect::new(area.x, area.y, area.width, 1);
    Paragraph::new(title_line).render(title_area, buf);
    Paragraph::new(close_hint)
        .alignment(Alignment::Right)
        .render(title_area, buf);

    // Row 2: Subtitle
    let subtitle = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            "Configure deployment target and runtime flags.",
            Style::default().fg(palette::TEXT_SECONDARY),
        ),
    ]);
    let subtitle_area = Rect::new(area.x, area.y + 1, area.width, 1);
    Paragraph::new(subtitle).render(subtitle_area, buf);
}
```

**4. Render separator lines:**

```rust
fn render_separator(area: Rect, buf: &mut Buffer) {
    let separator = "─".repeat(area.width as usize);
    buf.set_string(
        area.x,
        area.y,
        &separator,
        Style::default().fg(palette::BORDER_DIM),
    );
}
```

**5. Update `render_panes()` call:**

Pass `chunks[2]` (content area) instead of the full inner area to `render_panes()`.

**6. Update vertical layout similarly:**

Apply the same header/separator/footer structure in `render_vertical()`. For compact mode, the header can be reduced to 2 lines (title only, no subtitle).

#### Header Responsiveness

For narrow terminals (vertical layout, < 70 cols), use a compact header:
- 2 lines: title + close hint on one line, subtitle on second line
- If extremely narrow, skip subtitle

### Acceptance Criteria

1. Dialog block has no title on the border — title moved inside the header area
2. Header area shows "New Session" in `TEXT_BRIGHT` + `BOLD` (left-aligned)
3. Header shows "[Esc] Close" hint in `TEXT_MUTED` (right-aligned)
4. Header shows subtitle "Configure deployment target and runtime flags." in `TEXT_SECONDARY`
5. Horizontal separator line between header and content area using `BORDER_DIM`
6. Horizontal separator line between content and footer using `BORDER_DIM`
7. Content panes render in the correct area (between separators)
8. Both horizontal and vertical layouts include the header/separator structure
9. `cargo check -p fdemon-tui` passes
10. `cargo clippy -p fdemon-tui` passes

### Testing

- Visually verify header renders with title, subtitle, and close hint
- Verify separators are visible between header, content, and footer
- Test horizontal layout (100x40 terminal) — header should have room for title + close hint
- Test vertical layout (50x30 terminal) — compact header
- Verify content panes still receive correct area dimensions

### Notes

- **Header background**: The header area uses the same `POPUP_BG` as the rest of the modal. The design reference shows `bg-white/5` for the header, which is a very subtle brightening effect. In TUI, this can be approximated with `SURFACE` (Rgb(22,27,34)) which is slightly lighter than `POPUP_BG` (Rgb(28,33,43)). Or just keep the same background for simplicity.
- **Close button vs hint**: The TSX design shows an `X` button. In TUI, there's no clickable button, so we show `[Esc] Close` as a text hint.
- **Title sizing**: The TSX uses `text-xl` (large). In TUI, `Modifier::BOLD` is the best emphasis available. Consider using a simple prefix like `●` or icon to add visual weight.
