## Task: Redesign Settings Content Area

**Objective**: Transform the settings content area to match the Cyber-Glass design: group headers with icon + uppercase label in `ACCENT_DIM`, 3-column setting rows (label, value, description), and selected row indicator with left accent bar + tinted background.

**Depends on**: 02-update-settings-styles, 03-redesign-settings-header

### Scope

- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` — Redesign `render_content()`, `render_project_tab()`, `render_section_header()`, `render_setting_row()`, `render_user_prefs_tab()`, `render_user_pref_row()`, `render_launch_tab()`, `render_vscode_tab()`, `render_readonly_row()`
- `crates/fdemon-tui/src/widgets/settings_panel/styles.rs` — Use new style functions from task 02

### Details

#### Current Content Area

```
│ [Behavior]                                              │
│  ▶ Auto Start          true          Skip device...     │
│    Confirm Quit         true          Ask before...      │
│                                                         │
│ [Watcher]                                               │
│  ▶ Watch Paths          lib           Directories...    │
```

- Section headers: `[Section Name]` in `STATUS_YELLOW` + BOLD
- Rows: `▶ ` indicator + label + value + description
- Selected: `▶` indicator in `ACCENT`, label BOLD
- No background differentiation, no accent bar

#### Target Content Area

```
│                                                         │
│  ⚡ B E H A V I O R                                     │
│                                                         │
│ ▎ Auto Start          true*         Skip device...      │  ← selected: accent bar + bg tint
│   Confirm Quit         true          Ask before...      │  ← unselected: no bar, no bg
│                                                         │
│  👁 W A T C H E R                                       │
│                                                         │
│   Watch Paths          lib           Directories...     │
```

- Group headers: icon + spaced uppercase text in `ACCENT_DIM`
- Selected row: `▎` accent bar in `ACCENT` + `SELECTED_ROW_BG` background
- Unselected row: space instead of accent bar, no background
- Label: `TEXT_PRIMARY` (selected) or `TEXT_SECONDARY` (unselected)
- Value: type-colored (green/red for bool, accent for numbers, etc.)
- Description: `TEXT_MUTED` + italic

#### Implementation

**1. Redesign `render_section_header()` (currently lines 318-321):**

Replace `[Section Name]` format with icon + spaced uppercase:

```rust
fn render_section_header(
    x: u16,
    y: u16,
    width: u16,
    buf: &mut Buffer,
    section: &str,
    icons: &IconSet,
) {
    // Map section name to icon
    let icon = match section.to_lowercase().as_str() {
        "behavior" => icons.zap(),
        "watcher" => icons.eye(),
        "ui" | "ui preferences" => icons.monitor(),
        "devtools" => icons.cpu(),
        "editor" | "editor override" => icons.code(),
        "session memory" => icons.user(),
        _ => icons.settings(),
    };

    // Spaced uppercase: "Behavior" → "B E H A V I O R"
    let spaced: String = section
        .to_uppercase()
        .chars()
        .collect::<Vec<_>>()
        .join(" ");

    let icon_span = Span::styled(
        format!("  {} ", icon),
        styles::group_header_icon_style(),
    );
    let label_span = Span::styled(
        spaced,
        styles::section_header_style(), // Now returns ACCENT_DIM + BOLD
    );

    let line = Line::from(vec![icon_span, label_span]);
    buf.set_line(x, y, &line, width);
}
```

**2. Redesign `render_setting_row()` (currently lines 324-366):**

Replace `▶ ` indicator with `▎` accent bar + background tint:

```rust
fn render_setting_row(
    x: u16,
    y: u16,
    width: u16,
    buf: &mut Buffer,
    item: &SettingItem,
    is_selected: bool,
    is_editing: bool,
    edit_buffer: &str,
) {
    // Apply background for selected row
    if is_selected {
        let bg_style = styles::selected_row_bg();
        for col in x..x + width {
            if let Some(cell) = buf.cell_mut((col, y)) {
                cell.set_style(bg_style);
            }
        }
    }

    let mut col = x;

    // Column 0: Left accent bar (1 char)
    if is_selected {
        let bar = Span::styled("▎", styles::accent_bar_style());
        buf.set_line(col, y, &Line::from(bar), 1);
    }
    col += INDICATOR_WIDTH; // 3 chars total: bar + 2 spaces

    // Column 1: Label (LABEL_WIDTH chars)
    let label_text = styles::truncate_str(&item.label, LABEL_WIDTH as usize);
    let label_style = styles::label_style(is_selected);
    buf.set_string(col, y, &format!("{:<width$}", label_text, width = LABEL_WIDTH as usize), label_style);
    col += LABEL_WIDTH;

    // Column 2: Value (VALUE_WIDTH chars)
    if is_editing && is_selected {
        // Show edit buffer + cursor
        let display = format!("{}▌", edit_buffer);
        let truncated = styles::truncate_str(&display, VALUE_WIDTH as usize);
        buf.set_string(col, y, &format!("{:<width$}", truncated, width = VALUE_WIDTH as usize), styles::editing_style());
    } else {
        let display = item.value.display();
        let modified_marker = if item.is_modified() { "*" } else { "" };
        let display_with_marker = format!("{}{}", display, modified_marker);
        let truncated = styles::truncate_str(&display_with_marker, VALUE_WIDTH as usize);
        let val_style = styles::value_style(&item.value, is_selected);
        buf.set_string(col, y, &format!("{:<width$}", truncated, width = VALUE_WIDTH as usize), val_style);
    }
    col += VALUE_WIDTH;

    // Column 3: Description (remaining width, italic)
    let remaining = width.saturating_sub(col - x);
    if remaining > 3 {
        let desc = styles::truncate_str(&item.description, remaining as usize);
        buf.set_string(col, y, &desc, styles::description_style());
    }
}
```

**3. Update all tab renderers to pass `icons` parameter:**

Each tab renderer (`render_project_tab`, `render_user_prefs_tab`, `render_launch_tab`, `render_vscode_tab`) calls `render_section_header()`. They need to pass an `IconSet` instance:

```rust
let icons = IconSet::new(IconMode::Unicode); // Default for now
```

Create the `IconSet` once in `render_content()` and pass it down to all tab renderers.

**4. Update `render_user_pref_row()` (currently lines 484-555):**

Same accent bar pattern as `render_setting_row()`, but with the override indicator:

```rust
// Column 0: Left accent bar + override indicator
if is_selected {
    buf.set_string(col, y, "▎", styles::accent_bar_style());
}
col += 1;

// Override indicator (⚡ if override active)
if is_override_active {
    buf.set_string(col, y, "⚡", styles::override_indicator_style(true, is_selected));
}
col += 2; // Space for indicator + gap
```

The override `⚡` character sits between the accent bar and the label.

**5. Update `render_readonly_row()` (VSCode tab, currently lines 847-882):**

Same accent bar pattern but with dimmed styling:

- Selected: `▎` accent bar in `TEXT_MUTED` (dimmer since read-only)
- Label: `TEXT_PRIMARY` (selected) or `TEXT_SECONDARY` (unselected)
- Value: `TEXT_MUTED`
- Lock icon `🔒` still shown for selected row

**6. Update `render_config_header()` and `render_vscode_config_header()`:**

These headers for individual configurations within the Launch/VSCode tabs should also get the icon treatment, but using a different visual style:

```rust
fn render_config_header(x: u16, y: u16, width: u16, buf: &mut Buffer, section: &str) {
    // Format: "─── Configuration 1 ────────"
    // Keep current separator-line style but use ACCENT_DIM instead of ACCENT
    let header_style = styles::config_header_style(); // Already ACCENT, could change to ACCENT_DIM
}
```

Keep the current `─── Configuration N ───` format but ensure colors align with the overall design.

### Acceptance Criteria

1. Group headers render as: icon + spaced uppercase text in `ACCENT_DIM`
2. Group icon maps correctly to section name (Behavior→zap, Watcher→eye, UI→monitor, Editor→code, etc.)
3. Selected row shows `▎` left accent bar in `ACCENT` color
4. Selected row has subtle `SELECTED_ROW_BG` background fill
5. Unselected rows have no accent bar and no background
6. Labels use `TEXT_PRIMARY` when selected, `TEXT_SECONDARY` when unselected
7. Values retain type-based coloring (bool=green/red, number=accent, etc.)
8. Descriptions render in `TEXT_MUTED` + italic
9. User pref rows show `⚡` override indicator between accent bar and label
10. VSCode read-only rows use dimmed accent bar + lock icon
11. 3-column layout preserved: indicator(3) + label(25) + value(15) + description(flex)
12. Editing mode still works (edit buffer + cursor + yellow styling)
13. Modified marker `*` still displays on changed values
14. `cargo check -p fdemon-tui` passes
15. `cargo clippy -p fdemon-tui` passes

### Testing

- Verify group header icon mapping for all 5 project sections: Behavior, Watcher, UI, DevTools, Editor
- Verify group header icon mapping for User tab sections: Editor Override, UI Preferences, Session Memory
- Verify accent bar appears only on selected row
- Verify background tint on selected row doesn't obscure text
- Test editing mode: accent bar + yellow edit buffer should coexist
- Test with modified values: `*` marker still visible
- Test User tab override indicators: `⚡` shows correctly
- Test VSCode tab read-only rows: dimmed styling preserved
- Test scrolling: accent bar follows selection through long lists

### Notes

- **Spaced uppercase**: "Behavior" → "B E H A V I O R" — this is a common design pattern for small header text with wide letter-spacing. Implemented by joining chars with spaces.
- **Icon mapping**: The match on section names is string-based. If section names change in `settings_items.rs`, the mapping needs updating. Consider using constants for section names.
- **Content max-width**: The design shows `max-w-4xl mx-auto` (max 896px). In the terminal, the content already fills the panel width. Adding horizontal centering/padding for very wide terminals is optional polish — skip for now.
- **`IconSet` threading**: Currently, `IconSet` is created locally. In the future, it could be stored in `SettingsViewState` or passed from the render context. For now, create it in `render_content()`.
- **render_content border**: Currently uses `Borders::LEFT | Borders::RIGHT`. The glass container from the header block may make this redundant. Verify visually whether the content area needs its own side borders or if the outer block provides them.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` | Redesigned all content area renderers with Cyber-Glass design: group headers with icon + spaced uppercase, accent bar + background for selected rows, 3-column layout with italic descriptions |

### Notable Decisions/Tradeoffs

1. **Manual character spacing**: Used manual loop to create spaced uppercase text ("B E H A V I O R") instead of itertools `join()` to avoid adding a new dependency.
2. **IconSet threading**: Created `IconSet` once in `render_content()` and passed it down to all tab renderers to avoid repeated instantiation. Used `IconMode::Unicode` as default for now.
3. **User pref override layout**: Override indicator `⚡` now sits between accent bar and label (col 0: bar, col 1-2: override indicator space, col 3+: label) for cleaner alignment.
4. **Read-only row dimming**: VSCode tab uses `TEXT_MUTED` for accent bar instead of full `ACCENT` to visually distinguish read-only rows from editable ones.
5. **Background fill technique**: Applied `SELECTED_ROW_BG` by iterating over all columns in the row and setting cell background before rendering content, ensuring full-width tinting.

### Testing Performed

- `cargo check -p fdemon-tui` - Passed
- `cargo clippy -p fdemon-tui` - Passed (only 3 warnings about unused old style functions in styles.rs)
- Manual verification: Read code paths for all 4 tabs (Project, User, Launch, VSCode) and all row renderers

### Verification Checklist

- ✅ Group headers render as: icon + spaced uppercase text in `ACCENT_DIM`
- ✅ Group icon maps correctly to section name (Behavior→zap, Watcher→eye, UI→monitor, Editor→code, Session Memory→user)
- ✅ Selected row shows `▎` left accent bar in `ACCENT` color
- ✅ Selected row has `SELECTED_ROW_BG` background fill
- ✅ Unselected rows have no accent bar and no background
- ✅ Labels use `TEXT_PRIMARY` when selected, `TEXT_SECONDARY` when unselected
- ✅ Values retain type-based coloring (bool=green/red, number=accent, etc.)
- ✅ Descriptions render in `TEXT_MUTED` + italic
- ✅ User pref rows show `⚡` override indicator between accent bar and label
- ✅ VSCode read-only rows use dimmed accent bar (`TEXT_MUTED`) + lock icon
- ✅ 3-column layout preserved: indicator(3) + label(25) + value(15) + description(flex)
- ✅ Editing mode still works (edit buffer + cursor + yellow styling)
- ✅ Modified marker `*` still displays on changed values

### Risks/Limitations

1. **Icon mode hardcoded**: Currently using `IconMode::Unicode` hardcoded. Future tasks should wire this from `Settings.theme.icon_mode` config value.
2. **Unused style functions**: Old `indicator_style()`, `readonly_indicator_style()`, and `INDICATOR_WIDTH_OVERRIDE` constant are now unused. These can be removed in a cleanup task once all Phase 4 tasks are complete.
3. **Manual character spacing**: The spaced uppercase implementation is simple but not unicode-aware. If section names contain non-ASCII characters, spacing may not work as expected. This is acceptable since all current section names are ASCII.
