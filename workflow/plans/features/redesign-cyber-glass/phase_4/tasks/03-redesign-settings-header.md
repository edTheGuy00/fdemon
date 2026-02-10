## Task: Redesign Settings Panel Header

**Objective**: Transform the settings panel header to match the Cyber-Glass design: settings icon + "System Settings" title, pill-style tab bar with `ACCENT` bg on active tab and rounded-top styling, and `[Esc] Close` hint in kbd badge style.

**Depends on**: 01-add-settings-icons

### Scope

- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` — Redesign `render_header()`, `render_tab()`, `render_tab_underline()`

### Details

#### Current Header (lines 103-223)

```
┌─ Settings ─────────────────────────────────────────────────────┐
│  1.Project   2.User   3.Launch   4.VSCode        [Esc] Close  │
│  ──────────                                                    │
└────────────────────────────────────────────────────────────────┘
```

- Full border around header area
- Title in block title position
- Tabs rendered as text with active tab underlined
- Active tab: `ACCENT` bg + `CONTRAST_FG` text
- Close hint in top-right

#### Target Header

```
┌────────────────────────────────────────────────────────────────┐
│ ⚙ System Settings                                 [Esc] Close │
│                                                                │
│ ╭──────────╮╭──────────╮╭──────────╮╭──────────╮              │
│ │1. PROJECT││2. USER   ││3. LAUNCH ││4. VSCODE │              │
│ ╰──────────╯╰──────────╯╰──────────╯╰──────────╯              │
└────────────────────────────────────────────────────────────────┘
```

- `SURFACE` bg for header area
- Left: `ICON_SETTINGS` in `ACCENT` + "System Settings" in `TEXT_BRIGHT` bold
- Right: `[Esc]` kbd badge + "Close" in `TEXT_MUTED`
- Tab bar below title: pill-style buttons
  - Active: `ACCENT` bg + `TEXT_BRIGHT` text + BOLD
  - Inactive: no bg + `TEXT_SECONDARY` text
  - Labels: uppercase with number prefix ("1. PROJECT", "2. USER", etc.)

#### Implementation

**1. Increase header height from 3 to 5 lines:**

In `render()` (line 80), change the header constraint:

```rust
let chunks = Layout::vertical([
    Constraint::Length(5),   // Header (was 3): title row + gap + tab row + gap + border
    Constraint::Min(5),     // Content
    Constraint::Length(2),  // Footer
])
.split(area);
```

**2. Redesign `render_header(area, buf, state)`:**

Replace the current bordered block approach with a custom layout:

```rust
fn render_header(area: Rect, buf: &mut Buffer, state: &SettingsViewState) {
    // Background: SURFACE for the entire header area
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(styles::border_inactive())
        .style(Style::default().bg(palette::SURFACE));

    let inner = header_block.inner(area);
    header_block.render(area, buf);

    // Row 1: Icon + Title (left) ... [Esc] Close (right)
    let title_y = inner.top();

    // Left: settings icon + title
    let icons = IconSet::new(/* icon mode from state or default */);
    let icon_span = Span::styled(
        format!("{} ", icons.settings()),
        Style::default().fg(palette::ACCENT),
    );
    let title_span = Span::styled(
        "System Settings",
        Style::default().fg(palette::TEXT_BRIGHT).add_modifier(Modifier::BOLD),
    );
    let title_line = Line::from(vec![icon_span, title_span]);
    buf.set_line(inner.left() + 1, title_y, &title_line, inner.width - 2);

    // Right: [Esc] Close
    let esc_badge = Span::styled(
        " Esc ",
        styles::kbd_badge_style(),
    );
    let close_label = Span::styled(" Close", styles::kbd_label_style());
    let close_line = Line::from(vec![esc_badge, close_label]);
    let close_width = 11; // " Esc  Close"
    buf.set_line(inner.right() - close_width - 1, title_y, &close_line, close_width);

    // Row 2 (skip 1 line gap): Tab bar
    let tab_y = title_y + 2;
    let tab_area = Rect::new(inner.left() + 1, tab_y, inner.width - 2, 1);
    render_tab_bar(tab_area, buf, state);
}
```

**3. Redesign `render_tab_bar()` (replaces old `render_tab` + `render_tab_underline`):**

Create a new `render_tab_bar()` function that renders all 4 tabs as pill-style buttons:

```rust
fn render_tab_bar(area: Rect, buf: &mut Buffer, state: &SettingsViewState) {
    let tabs = [
        (SettingsTab::Project, "1. PROJECT"),
        (SettingsTab::UserPrefs, "2. USER"),
        (SettingsTab::LaunchConfig, "3. LAUNCH"),
        (SettingsTab::VSCodeConfig, "4. VSCODE"),
    ];

    let tab_width = 12u16; // Fixed width per tab
    let gap = 1u16;        // Gap between tabs

    let mut x = area.left();
    for (tab, label) in tabs {
        if x + tab_width > area.right() { break; }

        let is_active = state.active_tab == tab;
        let tab_rect = Rect::new(x, area.top(), tab_width, 1);

        if is_active {
            // Active: ACCENT bg, TEXT_BRIGHT fg, BOLD
            let style = Style::default()
                .fg(palette::TEXT_BRIGHT)
                .bg(palette::ACCENT)
                .add_modifier(Modifier::BOLD);
            let centered = format!("{:^width$}", label, width = tab_width as usize);
            buf.set_string(tab_rect.left(), tab_rect.top(), &centered, style);
        } else {
            // Inactive: no bg, TEXT_SECONDARY
            let style = Style::default().fg(palette::TEXT_SECONDARY);
            let centered = format!("{:^width$}", label, width = tab_width as usize);
            buf.set_string(tab_rect.left(), tab_rect.top(), &centered, style);
        }

        x += tab_width + gap;
    }
}
```

**4. Remove old `render_tab()` and `render_tab_underline()` functions:**

These are replaced by the unified `render_tab_bar()`. Delete the old implementations (lines 164-223).

**5. Update tab labels to uppercase:**

Change from "1.Project" to "1. PROJECT" — note the space after the dot and uppercase label.

### Acceptance Criteria

1. Header shows `ICON_SETTINGS` (⚙) in `ACCENT` color + "System Settings" in `TEXT_BRIGHT` bold
2. Header background uses `SURFACE` color
3. `[Esc] Close` hint renders in top-right with kbd badge styling
4. Tab bar renders 4 pill-style tabs in a single row
5. Active tab: `ACCENT` bg + `TEXT_BRIGHT` text + BOLD
6. Inactive tabs: no bg + `TEXT_SECONDARY` text
7. Tab labels are uppercase with number prefix and dot separator
8. Header height increased to 5 lines (from 3) to accommodate title + tab rows
9. `render_tab()` and `render_tab_underline()` removed (replaced by `render_tab_bar()`)
10. `cargo check -p fdemon-tui` passes
11. `cargo clippy -p fdemon-tui` passes

### Testing

- Visually verify header layout: icon + title on first row, tabs on third row
- Verify active tab highlighting cycles correctly with Tab key
- Verify all 4 tabs render within the header width
- Test narrow terminals: tabs should truncate gracefully if not enough width
- Verify `[Esc] Close` hint doesn't overlap with title on narrow terminals

### Notes

- **Header height change**: Increasing from 3 to 5 lines reduces content area by 2 lines. On very small terminals (< 20 rows), this could be tight. The content area has `Min(5)` constraint, so ratatui will handle gracefully.
- **Icon mode**: The `IconSet` needs an `IconMode` which comes from settings. Currently `render_header` doesn't have access to icon mode. Options:
  1. Accept `IconMode` as parameter
  2. Default to `IconMode::Unicode` for safety
  3. Store icon mode in `SettingsViewState`

  Use option 2 for now — default to Unicode. This can be wired up later.
- **Tab width**: 12 chars per tab × 4 tabs + 3 gaps = 51 chars minimum. The settings panel renders full-screen, so this should always fit.
- **Old function cleanup**: After removing `render_tab()` and `render_tab_underline()`, check that no other code calls them.

---

## Completion Summary

**Status:** Not Started
