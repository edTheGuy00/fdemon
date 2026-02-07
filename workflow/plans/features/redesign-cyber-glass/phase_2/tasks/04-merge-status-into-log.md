## Task: Merge Status Bar into Log View Footer

**Objective**: Integrate the status bar information into the log view's bottom metadata bar, replacing the standalone status bar widget. The design reference shows status info inside the log panel border, not as a separate bar.

**Depends on**: 03-redesign-log-view

### Scope

- `crates/fdemon-tui/src/widgets/log_view/mod.rs` — Add bottom metadata bar rendering
- `crates/fdemon-tui/src/widgets/status_bar/mod.rs` — Mark for removal or deprecation (actual removal in Task 05)
- `crates/fdemon-tui/src/render/mod.rs` — Stop rendering standalone StatusBar (deferred to Task 05 layout changes)

### Details

#### Current Status Bar (standalone, 2 rows)

```
────────────────────────────────────────────────────────
 ● Running | Debug (develop) | ⏱ 5:32 | ↻ 12 | ✓ No errors | ↓ Auto  42-60/120
```

Occupies 2 rows: 1 for top border separator, 1 for content. Shows:
- Phase indicator (icon + label + color)
- Config info (mode + optional flavor)
- Session timer
- Last reload time
- Error count
- Scroll indicator (Auto/Manual)
- Log position (start-end/total)

#### Target: Bottom Metadata Bar (inside log view, 1 line)

```
╭──────────────────────────────────────────────────╮
│  TERMINAL LOGS                         LIVE FEED │  ← top metadata bar
│ 12:34:56  •  [app] Hot reload completed          │
│ 12:34:57  •  [flutter] Reloaded 2 of 512 libs    │
│                                                  │
│ ● Running  Debug (develop)       ⏱ 5:32  ⚠ 0   │  ← bottom metadata bar
╰──────────────────────────────────────────────────╯
```

**Bottom metadata bar layout (1 line, inside glass container):**

**Left side:**
- Status dot: phase icon in phase color (from `theme::styles::phase_indicator()`)
- "Running" label in `STATUS_GREEN` (or corresponding phase color)
- Mode badge: "Debug (develop)" in `ACCENT` — flutter mode + optional flavor

**Right side:**
- Uptime: `ICON_ACTIVITY` + "5:32" in `TEXT_SECONDARY`
- Error count: `ICON_ALERT` + "0" in `TEXT_MUTED` (or `STATUS_RED` if errors > 0)

#### Implementation

**LogView needs AppState access:**

Currently `LogView` receives only `logs`, `filter_state`, `search_state`, `collapse_state`, `link_highlight_state`, and display options. The bottom metadata bar needs additional data:
- Phase (from session)
- Flutter mode (from session config)
- Flavor (from session config)
- Session duration
- Error count

Options:
1. **Add builder methods** to `LogView` for each new field
2. **Add a `StatusInfo` struct** that bundles the status data

Recommendation: Add a `StatusInfo` struct for cleanliness:

```rust
pub struct StatusInfo<'a> {
    pub phase: &'a AppPhase,
    pub is_busy: bool,
    pub mode: Option<&'a FlutterMode>,
    pub flavor: Option<&'a str>,
    pub duration: Option<Duration>,
    pub error_count: usize,
}
```

Then `LogView` gets a builder method:
```rust
pub fn with_status(mut self, status: StatusInfo<'a>) -> Self {
    self.status_info = Some(status);
    self
}
```

**Rendering the bottom metadata bar:**

```rust
// Inside StatefulWidget::render, after computing inner area:
let inner = block.inner(area);

// Top metadata bar: 1 line
let meta_top = Rect::new(inner.x, inner.y, inner.width, 1);

// Bottom metadata bar: 1 line (only if status_info is present)
let has_footer = self.status_info.is_some();
let footer_height = if has_footer { 1 } else { 0 };
let meta_bottom = if has_footer {
    Some(Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1))
} else {
    None
};

// Log content area: between the two bars
let content_area = Rect::new(
    inner.x,
    inner.y + 1,
    inner.width,
    inner.height.saturating_sub(1 + footer_height),
);

// Render metadata bars
render_top_metadata(meta_top, buf);
if let (Some(area), Some(status)) = (meta_bottom, &self.status_info) {
    render_bottom_metadata(area, buf, status);
}

// Render log entries in content_area
// visible_lines = content_area.height as usize
```

**`render_bottom_metadata()` implementation:**

```rust
fn render_bottom_metadata(area: Rect, buf: &mut Buffer, status: &StatusInfo) {
    let (icon, label, style) = if status.is_busy {
        theme::styles::phase_indicator_busy()
    } else {
        theme::styles::phase_indicator(status.phase)
    };

    let mut spans = vec![
        Span::raw(" "),
        Span::styled(icon, style),
        Span::raw(" "),
        Span::styled(label, style),
    ];

    // Mode badge
    if let Some(mode) = status.mode {
        let mode_text = match mode {
            FlutterMode::Debug => "Debug",
            FlutterMode::Profile => "Profile",
            FlutterMode::Release => "Release",
        };
        spans.push(Span::raw("  "));
        spans.push(Span::styled(mode_text, styles::accent()));
        if let Some(flavor) = status.flavor {
            spans.push(Span::styled(format!(" ({})", flavor), styles::text_secondary()));
        }
    }

    // Right-aligned: uptime + errors
    // Calculate right section width, then pad
    let mut right_spans = Vec::new();
    if let Some(duration) = status.duration {
        let mins = duration.as_secs() / 60;
        let secs = duration.as_secs() % 60;
        right_spans.push(Span::styled(
            format!("{} {}:{:02}", icons::ICON_ACTIVITY, mins, secs),
            styles::text_secondary(),
        ));
    }
    right_spans.push(Span::raw("  "));
    if status.error_count > 0 {
        right_spans.push(Span::styled(
            format!("{} {}", icons::ICON_ALERT, status.error_count),
            styles::status_red().add_modifier(Modifier::BOLD),
        ));
    } else {
        right_spans.push(Span::styled(
            format!("{} 0", icons::ICON_ALERT),
            styles::text_muted(),
        ));
    }

    // Calculate padding between left and right sections
    let left_width: usize = spans.iter().map(|s| s.width()).sum();
    let right_width: usize = right_spans.iter().map(|s| s.width()).sum();
    let padding = (area.width as usize).saturating_sub(left_width + right_width + 1);
    spans.push(Span::raw(" ".repeat(padding)));
    spans.extend(right_spans);

    let line = Line::from(spans);
    buf.set_line(area.x, area.y, &line, area.width);
}
```

#### Compact Mode

For terminals < 60 columns, the bottom metadata bar should show a minimal version:
- Phase icon + mode only (no uptime/errors)
- Or hide the bottom bar entirely and keep the status in a reduced form

The compact status bar logic from `StatusBarCompact` can inform this.

#### Caller Update (render/mod.rs)

The `view()` function needs to:
1. Build the `StatusInfo` struct from the active session
2. Pass it to `LogView::with_status()`
3. Stop rendering the standalone `StatusBar` / `StatusBarCompact` (deferred to Task 05)

For now, both can render simultaneously during development. Task 05 removes the standalone status bar and reclaims its 2 rows for the log view.

### Acceptance Criteria

1. `LogView` has a `StatusInfo` struct and `with_status()` builder method
2. Bottom metadata bar renders inside the log view glass container
3. Left side shows: phase indicator + mode badge + optional flavor
4. Right side shows: uptime timer + error count
5. Error count is red when > 0, muted when 0
6. Phase indicator uses consolidated `theme::styles::phase_indicator()` from Phase 1
7. Compact mode (< 60 cols) shows a simplified footer
8. `visible_lines` correctly accounts for both top and bottom metadata bars
9. All scroll calculations remain correct
10. `cargo check -p fdemon-tui` passes
11. `cargo clippy -p fdemon-tui` passes

### Testing

- Verify bottom bar renders with correct phase/mode/timer/error info
- Verify scroll behavior still works (visible lines reduced by 2 — top + bottom bars)
- Verify error count color changes at threshold (0 vs > 0)
- Test with no session (no footer bar, just top metadata bar)
- Test compact terminal width

### Notes

- **Don't remove the standalone StatusBar yet** — Task 05 handles the layout change that reclaims the status bar rows. During development, both can coexist.
- **Scroll indicator removal**: The current status bar shows "Auto"/"Manual" scroll mode. This info may not fit in the compact footer. Consider removing it — the scrollbar thumb position already indicates scroll state.
- **Log position removal**: The "42-60/120" position indicator may also be dropped from the footer. The scrollbar provides this visually.
- **Data plumbing**: The `view()` function already has access to `state.session_manager.selected()` which contains all the session data needed for `StatusInfo`. Building the struct is straightforward.
