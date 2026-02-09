## Task: Redesign Target Selector (Left Panel)

**Objective**: Transform the left panel of the New Session dialog to match the Cyber-Glass design: a pill-style tab toggle for Connected/Bootable, categorized device list with platform icons and uppercase headers, and themed selection highlighting.

**Depends on**: 02-redesign-modal-overlay, 03-redesign-modal-frame

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` — Redesign layout, category headers, device rendering
- `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs` — Redesign as pill-style toggle
- `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs` — Update selection styles, add platform icons

### Details

#### Current Target Selector

```
┌─ Target Selector ───────────┐
│ ┌──────────┬───────────┐    │
│ │Connected │ Bootable  │    │
│ └──────────┴───────────┘    │
│                              │
│   iOS Devices                │
│ ▶ iPhone 15 (physical)       │
│   iPad mini (simulator)      │
│                              │
│   Android Devices            │
│   Pixel 8 (physical)         │
│                              │
│ [↑↓] Navigate  [Enter] Select│
└──────────────────────────────┘
```

- Has its own border with "Target Selector" title
- Tab bar uses full bordered cells
- Category headers in `STATUS_YELLOW` + BOLD
- Selected device: `CONTRAST_FG` on `ACCENT` (black on cyan)

#### Target Design

```
│                              │
│ ┌──────────────────────────┐ │
│ │ 1 Connected │ 2 Bootable │ │  ← pill toggle (dark bg, active=ACCENT bg)
│ └──────────────────────────┘ │
│                              │
│   I O S  S I M U L A T O R S│  ← uppercase header in ACCENT_DIM
│                              │
│  📱 iPad mini (A17 Pro)  ◀  │  ← selected: ACCENT bg at 10% + border
│                              │
│  📱 iPhone 15 Pro           │  ← unselected: TEXT_SECONDARY
│                              │
│   W E B                      │
│                              │
│  🌐 Chrome                  │
│                              │
```

- No separate border — panel is a zone within the modal
- Right border: vertical separator line in `BORDER_DIM`
- Width: 40% of modal (currently 50%, change to 40/60 split)
- Pill-style tab toggle with dark background
- Category headers: uppercase, bold, `ACCENT_DIM`
- Device icons based on platform type
- Selected device: `ACCENT` bg tint + lighter text

#### Implementation

**1. Remove Target Selector's own border:**

Currently `target_selector.rs` renders its own `Block` with borders. Remove the border — the panel is part of the modal body, separated only by a vertical line.

Instead of a bordered block, render a right-side vertical separator:

```rust
// After rendering device content, draw right border
for y in area.top()..area.bottom() {
    let x = area.right().saturating_sub(1);
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_char('│');
        cell.set_style(Style::default().fg(palette::BORDER_DIM));
    }
}
```

Or use the Layout system to include a 1-col separator between panes in `mod.rs`.

**2. Update pane split in `mod.rs::render_panes()`:**

Change from 50/50 to 40/60 split:

```rust
let chunks = Layout::horizontal([
    Constraint::Percentage(40),  // Target Selector
    Constraint::Percentage(60),  // Launch Context
])
.split(area);
```

**3. Redesign TabBar as pill toggle (`tab_bar.rs`):**

Replace bordered tab cells with pill-style toggle:

```rust
impl Widget for TabBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Outer container: dark background with padding
        let container_bg = palette::DEEPEST_BG;  // Very dark
        let container_block = Block::default()
            .style(Style::default().bg(container_bg))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM));

        let inner = container_block.inner(area);
        container_block.render(area, buf);

        // Split into two equal halves
        let tabs = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]).split(inner);

        // Render each tab
        for (i, tab) in [TargetTab::Connected, TargetTab::Bootable].iter().enumerate() {
            let is_active = *tab == self.active_tab;
            let label = match tab {
                TargetTab::Connected => "1 Connected",
                TargetTab::Bootable => "2 Bootable",
            };

            let style = if is_active && self.pane_focused {
                Style::default()
                    .fg(palette::TEXT_BRIGHT)
                    .bg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(palette::TEXT_SECONDARY)
            };

            let paragraph = Paragraph::new(label)
                .style(style)
                .alignment(Alignment::Center);
            paragraph.render(tabs[i], buf);
        }
    }
}
```

**4. Redesign category headers (`device_list.rs`):**

Replace `STATUS_YELLOW` headers with `ACCENT_DIM` uppercase style:

```rust
DeviceListItem::Header(header) => {
    let header_style = Style::default()
        .fg(palette::ACCENT_DIM)
        .add_modifier(Modifier::BOLD);

    // Uppercase the header text
    let header_upper = header.to_uppercase();

    ListItem::new(Line::from(vec![
        Span::raw("  "),
        Span::styled(header_upper, header_style),
    ]))
}
```

**5. Add platform icons to device rows (`device_list.rs`):**

Add platform-appropriate icons before device names:

```rust
use crate::theme::icons;

fn device_icon(platform: &str, is_emulator: bool) -> &'static str {
    if platform.contains("ios") || platform.contains("simulator") {
        icons::current().smartphone()
    } else if platform.contains("web") || platform.contains("chrome") {
        icons::current().globe()
    } else if platform.contains("macos") || platform.contains("linux") || platform.contains("windows") {
        icons::current().monitor()
    } else {
        icons::current().cpu()
    }
}
```

Note: The `Device` struct has `platform_type` or similar field — check the actual field name. If platform info is only in the category header, icons can be derived from the category group name instead.

**6. Update device selection highlighting:**

Replace the current `CONTRAST_FG` on `ACCENT` (black on bright) with a subtler selected style:

```rust
let style = if is_selected && self.is_focused {
    Style::default()
        .fg(palette::TEXT_BRIGHT)
        .bg(palette::ACCENT)
        .add_modifier(Modifier::BOLD)
} else if is_selected {
    Style::default()
        .fg(palette::ACCENT)
        .add_modifier(Modifier::BOLD)
} else {
    Style::default()
        .fg(palette::TEXT_SECONDARY)
};
```

**7. Update compact mode:**

Compact tab bar (single-line inline format) should also use the pill styling approach but condensed:

```
[1 Connected] 2 Bootable
```

Active tab gets `ACCENT` fg + BOLD, inactive gets `TEXT_MUTED`.

### Acceptance Criteria

1. Target Selector has no separate border — uses vertical separator line to divide from Launch Context
2. Tab toggle renders as pill-style with dark background container
3. Active tab: `ACCENT` bg + `TEXT_BRIGHT` text when focused, `ACCENT` text when unfocused
4. Inactive tab: `TEXT_SECONDARY` text
5. Category headers render in uppercase with `ACCENT_DIM` color
6. Device rows show platform icons (smartphone/globe/monitor/cpu)
7. Selected device uses themed highlighting (not plain black-on-cyan)
8. Pane split changed from 50/50 to 40/60
9. Both horizontal and vertical layouts work correctly
10. Scroll indicators still render correctly
11. Loading/error states still render correctly
12. `cargo check -p fdemon-tui` passes
13. `cargo clippy -p fdemon-tui` passes

### Testing

- Visually verify tab toggle styling (active vs inactive, focused vs unfocused)
- Verify category headers are uppercase in `ACCENT_DIM`
- Verify device icons render (Nerd Fonts and Unicode fallback)
- Test device selection navigation (up/down, tab switch)
- Test in horizontal layout — verify 40/60 split
- Test in vertical layout — verify compact tab rendering
- Test with no devices — verify empty state message still displays
- Test with many devices — verify scroll indicators work

### Notes

- **Platform detection**: The `Device` struct from `fdemon-daemon` has fields like `platform_type` (String). The grouping logic in `device_groups.rs` already categorizes by platform — leverage this for icon selection.
- **Bootable device icons**: For `BootableDeviceList`, iOS simulators → smartphone icon, Android AVDs → smartphone icon (they're all mobile emulators).
- **Tab bar height**: The redesigned tab bar with a container block needs 3 lines (border + content + border). Ensure the layout allocates enough space.
- **Separator approach**: Two options for the vertical separator:
  1. Render it in `target_selector.rs` at the right edge
  2. Add a 1-col separator column in `mod.rs::render_panes()` layout

  Option 2 is cleaner — use a 3-column layout: `[40% | 1 col | 60%]`.
