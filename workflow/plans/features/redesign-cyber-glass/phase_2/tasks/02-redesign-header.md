## Task: Redesign MainHeader Widget

**Objective**: Transform the header from a plain bordered box with title-on-border to a Cyber-Glass styled container with a pulsing status dot, project name, keyboard shortcut hints, device pill, and integrated session tabs.

**Depends on**: None (Phase 1 theme module must exist)

### Scope

- `crates/fdemon-tui/src/widgets/header.rs` — Complete redesign of `MainHeader::render()`
- `crates/fdemon-tui/src/widgets/tabs.rs` — Update `SessionTabs` styling, fix icon color bug

### Details

#### Current Header Layout (3 rows)

```
Row 0 (border): " Flutter Demon  |  my_app          [r] [R] [x] [d] [q]"
Row 1 (content): "  ● iPhone 15  │ ● Pixel 8                           "
Row 2 (border): "╰──────────────────────────────────────────────────────╯"
```

Title and keybindings overwrite the top border. Session tabs occupy the 1-row content area.

#### Target Header Layout (3 rows)

```
Row 0 (border): "╭──────────────────────────────────────────────────────╮"
Row 1 (content): " ● Flutter Demon / my_app   [r] [R] [x] [d] [q]  📱 iPhone 15 "
Row 2 (border): "╰──────────────────────────────────────────────────────╯"
```

For multi-session (taller header if needed, or overflow tabs into a second row):
```
Row 0 (border): "╭──────────────────────────────────────────────────────╮"
Row 1 (content): " ● Flutter Demon / my_app   [r] [R] [x] [d] [q]  📱 iPhone 15 "
Row 2 (tabs):    "  ● iPhone 15  │ ● Pixel 8                           "
Row 3 (border): "╰──────────────────────────────────────────────────────╯"
```

#### Redesign Specification

**Glass container:**
- `Block` with `Borders::ALL`, `BorderType::Rounded`, `BORDER_DIM` border color
- Background: `CARD_BG`

**Content row layout (left → center → right):**

**Left section:**
- Status dot: `ICON_DOT` (`●`) in `STATUS_GREEN` (when running) — animate with `Modifier::SLOW_BLINK` or tick-based if pulsing is desired
- "Flutter Demon" in `ACCENT` + `BOLD`
- "/" separator in `TEXT_MUTED`
- Project name in `TEXT_SECONDARY`

**Center section:**
- Keyboard shortcut hints: `[r] Run  [R] Restart  [x] Stop  [d] Debug  [q] Quit`
- Brackets `[]` in `TEXT_MUTED`
- Key letters in `STATUS_YELLOW`
- Labels in `TEXT_MUTED`
- Show all dimmed (no hover concept in TUI)

**Right section (device pill):**
- Device icon based on platform:
  - iOS/simulator → `ICON_SMARTPHONE`
  - Web/Chrome → `ICON_GLOBE`
  - Desktop/macOS/linux/windows → `ICON_MONITOR`
  - Unknown → `ICON_CPU`
- Device name in `ACCENT`
- Optional: wrap in a subtle rounded container (simulated with spaces + bg color)

#### Implementation Approach

**`MainHeader::render()` rewrite:**

1. Render the glass block: `styles::glass_block(false)` with `.style(Style::default().bg(palette::CARD_BG))`
2. Compute `inner = block.inner(area)` — content area inside borders
3. Split inner horizontally:
   - Left: fixed width for status dot + title + project name
   - Center: flexible for shortcut hints
   - Right: fixed width for device pill
4. Render each section with themed styles

**Platform detection for device icon:**

The device platform info is available via `session.platform` (from `SessionHandle`). Map to icon:

```rust
fn device_icon(platform: Option<&str>) -> &'static str {
    match platform {
        Some(p) if p.contains("ios") || p.contains("simulator") => icons::ICON_SMARTPHONE,
        Some(p) if p.contains("web") || p.contains("chrome") => icons::ICON_GLOBE,
        Some(p) if p.contains("macos") || p.contains("linux") || p.contains("windows") => icons::ICON_MONITOR,
        _ => icons::ICON_CPU,
    }
}
```

**Status dot color:**

Use the phase indicator from Task 04 (Phase 1):
```rust
let (icon, _label, style) = theme::styles::phase_indicator(&session.phase);
```

#### Session Tabs Update (`tabs.rs`)

**Fix icon color bug:** In `tab_titles()` (line 34), the `_icon_color` is computed but never applied. Fix this by creating styled `Span`s instead of a plain `format!()`:

```rust
// Before (line 46)
Line::from(format!(" {} {} ", icon, name))

// After
Line::from(vec![
    Span::raw(" "),
    Span::styled(icon, style),
    Span::raw(format!(" {} ", name)),
])
```

**Update tab highlight style:** Replace `Color::Black, bg: Color::Cyan` with `palette::` references.

**Multi-session layout:** When multiple sessions exist, tabs render in the content row(s) below the title line. If the header height is still 3 rows, the single content row must accommodate both title info AND tabs — which means for multi-session we may need to split the title/shortcuts into the top border line (current approach) or expand to 4 rows.

**Recommendation:** Keep the current 3-row header for single sessions (title + shortcuts + device pill all in one content row). For multi-session, the content row shows session tabs, and the title/shortcuts move to the border line (similar to current behavior). This minimizes layout disruption.

### Acceptance Criteria

1. Header renders as a glass container (`BorderType::Rounded`, `CARD_BG` bg, `BORDER_DIM` border)
2. Left section shows status dot (colored by phase) + "Flutter Demon" in accent + project name
3. Shortcut hints displayed with themed colors (`TEXT_MUTED` brackets, `STATUS_YELLOW` keys)
4. Device pill shows platform icon + device name in `ACCENT` (right-aligned)
5. Session tabs use phase-colored icons (fix existing `_icon_color` bug)
6. Tab highlight uses theme palette colors
7. Single-session and multi-session modes both render correctly
8. `cargo check -p fdemon-tui` passes
9. `cargo clippy -p fdemon-tui` passes

### Testing

- Visually verify single-session header (title + shortcuts + device pill)
- Visually verify multi-session header (tabs with phase icons)
- Test narrow terminal (< 60 cols) — shortcuts should be hidden or abbreviated
- Test with no sessions (startup state) — show title only

### Notes

- **`HeaderWithTabs` legacy widget**: The `HeaderWithTabs` struct and its 3 free functions (`render_tabs_header`, `render_simple_header`, `render_single_session_header`) in `tabs.rs` are not used by the main render pipeline. Consider removing them to reduce confusion, or leave as-is and focus on `MainHeader` + `SessionTabs`.
- **Pulsing dot**: True animation requires tick-based state changes. For now, use `Modifier::SLOW_BLINK` on the dot character. A proper tick-based pulse can be added in Phase 5 polish.
- **Device pill rounding**: True rounded container isn't possible in TUI. Simulate with spaces around the text and optionally a subtle background color difference. Or just render `icon name` without a container.
- **Header height**: Keep at 3 rows. If the design feels too cramped, Task 05 (layout) can increase it.
