## Task: Update Layout for Redesigned Widgets

**Objective**: Adjust the layout proportions to fit the redesigned header and log panel, reclaim the standalone status bar area (now merged into the log view footer), and add visual breathing room between sections.

**Depends on**: 02-redesign-header, 03-redesign-log-view, 04-merge-status-into-log

### Scope

- `crates/fdemon-tui/src/layout.rs` — Update `create_with_sessions()`, `ScreenAreas` struct
- `crates/fdemon-tui/src/render/mod.rs` — Remove standalone StatusBar rendering, update `view()` to pass `StatusInfo` to LogView

### Details

#### Current Layout (24-row terminal)

```
Row  0-2:  Header  (3 rows)  — Constraint::Length(3)
Row  3-21: Logs    (19 rows) — Constraint::Min(3)
Row 22-23: Status  (2 rows)  — Constraint::Length(2)
```

#### Target Layout (24-row terminal)

```
Row  0-2:  Header  (3 rows)  — Constraint::Length(3)
Row  3:    Gap     (1 row)   — visual breathing room (DEEPEST_BG shows through)
Row  4-23: Logs    (20 rows) — Constraint::Min(3), now includes top+bottom metadata bars
```

The standalone status bar is eliminated. The log view gains 2 rows (from the removed status bar) minus 1 row (for the gap), netting +1 row of log content. With the top and bottom metadata bars inside the log view consuming 2 rows of inner space, the effective log content area changes from 17 lines (19 inner - 2 borders) to 16 lines (20 - 2 borders - 2 metadata bars). This is a minor net reduction but the visual quality improvement is worth it.

**Alternative (no gap):**
```
Row  0-2:  Header  (3 rows)
Row  3-23: Logs    (21 rows) — gets all remaining space
```

This gives 17 effective content lines (21 - 2 borders - 2 metadata bars), same as current. Recommendation: try with gap first; remove gap if it feels too cramped.

#### Layout Changes

**`ScreenAreas` struct update:**

```rust
pub struct ScreenAreas {
    pub header: Rect,
    pub logs: Rect,
    // Remove: pub tabs: Option<Rect> (vestigial, always None)
    // Remove: pub status: Rect (merged into log view)
}
```

Or keep `status` as `Option<Rect>` for backward compatibility during transition.

**`create_with_sessions()` update:**

```rust
pub fn create_with_sessions(area: Rect, session_count: usize) -> ScreenAreas {
    let _ = session_count;

    let chunks = Layout::vertical([
        Constraint::Length(3),    // Header (glass container)
        Constraint::Length(1),    // Gap (breathing room)
        Constraint::Min(3),      // Logs (glass container with metadata bars)
    ])
    .split(area);

    ScreenAreas {
        header: chunks[0],
        logs: chunks[2],
    }
}
```

**`use_compact_status()` update:**

This function is used to decide between `StatusBar` and `StatusBarCompact`. Since we're merging status into the log view, this function can be repurposed to control whether the log footer shows a compact or full status line:

```rust
/// Whether to use compact mode for the integrated status footer.
pub fn use_compact_footer(area: Rect) -> bool {
    area.width < MIN_FULL_STATUS_WIDTH
}
```

#### Render Module Changes (`render/mod.rs`)

**Remove standalone status bar rendering:**

```rust
// Before (lines 57-61)
if layout::use_compact_status(area) {
    frame.render_widget(StatusBarCompact::new(state), areas.status);
} else {
    frame.render_widget(StatusBar::new(state), areas.status);
}

// After: Remove these lines entirely
```

**Build and pass StatusInfo to LogView:**

```rust
// In view(), when building the LogView
let log_view = if let Some(handle) = state.session_manager.selected() {
    let session = &handle.session;
    let status_info = StatusInfo {
        phase: &session.phase,
        is_busy: session.is_busy,
        mode: session.flutter_mode.as_ref(),
        flavor: session.flavor.as_deref(),
        duration: session.duration(),
        error_count: session.error_count,
    };

    LogView::new(&session.logs)
        .with_status(status_info)
        .show_timestamps(true)
        // ... other builder calls ...
} else {
    LogView::new(&VecDeque::new())
};
```

**Update inline overlay position calculations:**

The search bar, mini search status, and link highlight bar are currently positioned relative to `areas.logs`:
```rust
// Current (line 94-98)
let search_area = Rect::new(
    areas.logs.x + 1,
    areas.logs.y + areas.logs.height - 2,
    areas.logs.width - 2,
    1,
);
```

These positions should still work since they reference the logs area, which now includes the full log panel with metadata bars. The `-2` offset places the bar just above the bottom border, which is above the bottom metadata bar. This might need adjustment — the bar should appear above the bottom metadata bar, not overlap it.

**Adjusted overlay position:**

```rust
// If bottom metadata bar is at (logs.y + logs.height - 2), the overlay should be at -3
let search_area = Rect::new(
    areas.logs.x + 1,
    areas.logs.y + areas.logs.height - 3, // -3 to be above bottom metadata bar
    areas.logs.width - 2,
    1,
);
```

Or better: have the LogView expose the content area rect so overlays can position relative to it.

#### Cleanup

**Dead code in `layout.rs`:**

Several functions are marked `#[allow(dead_code)]`:
- `LayoutMode` enum and `from_width()`
- `create()` (non-sessions variant)
- `use_compact_header()`
- `header_height()`
- `max_visible_tabs()`

Recommendation: keep `LayoutMode` and `timestamp_format()` (useful for log entry formatting). Remove the rest if unused.

**StatusBar module:**

The `StatusBar` and `StatusBarCompact` widgets are no longer rendered by `view()`. Options:
1. Delete `status_bar/mod.rs` and `status_bar/tests.rs` entirely
2. Keep but deprecate (mark `#[deprecated]`)
3. Keep for potential future use (e.g., headless mode status)

Recommendation: keep the module for now but remove the `pub use` from `widgets/mod.rs`. This avoids accidental use while preserving the code for reference.

### Acceptance Criteria

1. Layout splits terminal into: header (3 rows) + gap (1 row) + logs (remaining)
2. No standalone status bar is rendered
3. `StatusInfo` is built in `view()` and passed to `LogView::with_status()`
4. Inline overlays (search, link highlight) are positioned correctly above the bottom metadata bar
5. Compact terminal (< 60 cols) still works
6. Very small terminal (< 20 rows) degrades gracefully (metadata bars hidden if needed)
7. `ScreenAreas` struct no longer has a `status` field (or it's optional/unused)
8. `cargo check -p fdemon-tui` passes
9. `cargo clippy -p fdemon-tui` passes

### Testing

- Verify layout at various terminal sizes (80x24, 120x40, 40x15, 200x50)
- Verify the gap between header and log panel is visible
- Verify inline overlays don't overlap the bottom metadata bar
- Verify the log content area has correct visible line count

### Notes

- **Gap row consideration**: The 1-row gap costs vertical space. If it feels too tight on 24-row terminals, remove the gap and let the header and log panel borders touch. The color difference (`CARD_BG` containers on `DEEPEST_BG` background) still provides visual separation.
- **Session data availability**: The `view()` function has `&mut AppState`, so all session data is accessible. No new data plumbing is needed.
- **Overlay positioning**: The search/link overlays currently use hardcoded offsets from the log area bounds. A cleaner approach would be for the LogView to report its content area bounds, but that requires passing information back through the render call — not straightforward with ratatui's widget model. The hardcoded offset approach is pragmatic.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/layout.rs` | Removed `status` field from `ScreenAreas` struct, updated `create_with_sessions()` to add 1-row gap between header and logs, renamed `use_compact_status()` to `use_compact_footer()`, updated all tests to reflect new layout proportions |
| `crates/fdemon-tui/src/render/mod.rs` | Removed standalone status bar rendering (lines 87-91), updated overlay positioning in `SearchInput`, `Normal`, and `LinkHighlight` modes to use `-3` offset (above bottom metadata bar instead of `-2`) |
| `crates/fdemon-tui/src/render/snapshots/*.snap` | Updated 4 snapshot files to reflect new layout (gap row visible, no standalone status bar) |

### Notable Decisions/Tradeoffs

1. **Gap implementation**: Added 1-row gap (Constraint::Length(1)) between header and logs to provide visual breathing room. The gap shows DEEPEST_BG background color, creating clear separation between glass containers.

2. **Overlay positioning**: Changed overlay y-position from `height - 2` to `height - 3` to position search/link bars above the bottom metadata bar. The metadata bar is at `height - 2`, so overlays must be at `height - 3`.

3. **ScreenAreas simplification**: Removed the `status` field entirely rather than making it `Option<Rect>`, as the standalone status bar is permanently replaced by the integrated metadata bars in LogView.

4. **use_compact_footer function**: Marked with `#[allow(dead_code)]` as it's currently unused but provided for future enhancement when the log view might want to conditionally format the status footer based on terminal width.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-tui` - Passed
- `cargo test -p fdemon-tui` - Passed (473/474 tests, 1 pre-existing failure in `test_header_with_keybindings` unrelated to layout changes)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed
- Snapshot tests updated via `cargo insta test --accept` for 4 render snapshots

### Layout Verification

On a 24-row terminal:
- Header: rows 0-2 (3 rows)
- Gap: row 3 (1 row, shows DEEPEST_BG)
- Logs: rows 4-23 (20 rows, includes top+bottom metadata bars internally)

Effective log content area: 16 lines (20 rows - 2 borders - 2 metadata bars)
Previous effective area: 17 lines (19 rows - 2 borders, before metadata bars)

Net change: -1 line of log content, but significantly improved visual hierarchy and information density through integrated metadata bars.

### Risks/Limitations

1. **Pre-existing test failure**: The `test_header_with_keybindings` test was already failing before these changes. This test checks for keybindings display in the header widget and is unrelated to layout proportions.

2. **Overlay positioning assumption**: The overlay positioning assumes the bottom metadata bar is always at `height - 2`. If the LogView layout changes internally, these offsets may need adjustment. A future enhancement could have LogView expose its content area bounds.

3. **Gap on small terminals**: On very small terminals (< 20 rows), the 1-row gap may feel wasteful. The layout could be enhanced to conditionally remove the gap when `area.height < 20`.
