## Task: Redesign LogView Widget

**Objective**: Transform the log view from a plain bordered panel to a Cyber-Glass styled container with a top metadata bar, styled log entries with colored source tags, and a blinking cursor line.

**Depends on**: None (Phase 1 theme module must exist)

### Scope

- `crates/fdemon-tui/src/widgets/log_view/mod.rs` — Redesign the glass container, add top metadata bar, restyle log entries

### Details

#### Current LogView Layout

```
╭ Logs ────────────────────────────────────────────╮
│ 12:34:56 ✓ [app] Hot reload completed            │
│ 12:34:57 • [flutter] Reloaded 2 of 512 libraries │
│                                                  ▲│
│                                                  ││
│                                                  ▼│
╰──────────────────────────────────────────────────╯
```

- Plain block with `Borders::ALL`, border fg `DarkGray`
- Title " Logs " on top border
- Log entries: `timestamp icon [source] message`
- Scrollbar overlays right border

#### Target LogView Layout

```
╭──────────────────────────────────────────────────╮
│  TERMINAL LOGS                         LIVE FEED │  ← top metadata bar
│──────────────────────────────────────────────────│
│ 12:34:56  •  [app] Hot reload completed          │
│ 12:34:57  •  [flutter] Reloaded 2 of 512 libs    │
│                                                  │
│ █                                                │  ← blinking cursor
╰──────────────────────────────────────────────────╯
```

- Glass container: `CARD_BG` background, `BorderType::Rounded`, `BORDER_DIM` border
- Top metadata bar (1 line inside border): `ICON_TERMINAL` + "TERMINAL LOGS" left, "LIVE FEED" badge right
- Separator line below metadata bar (optional — or just use spacing)
- Styled log entries with colored source tags
- Blinking cursor at end of content

#### Redesign Specification

**Glass container:**
- `Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)`
- `.border_style(Style::default().fg(palette::BORDER_DIM))`
- `.style(Style::default().bg(palette::CARD_BG))`
- Remove title from block (metadata bar replaces it)

**Top metadata bar (1 line, inside the glass container):**
- Left: `ICON_TERMINAL` + " TERMINAL LOGS" in `TEXT_SECONDARY`, uppercase
- Right: "LIVE FEED" badge — text in `TEXT_MUTED` with optional darker background (simulate with `DEEPEST_BG` bg)
- The badge can be rendered as `Span::styled(" LIVE FEED ", Style::default().fg(palette::TEXT_MUTED).bg(palette::DEEPEST_BG))`
- This consumes 1 line from the inner area, so the log content area starts 1 line lower

**Log entry styling (per line):**

| Component | Style |
|-----------|-------|
| Timestamp | `TEXT_MUTED` (unchanged semantic, now uses palette) |
| Bullet separator | `TEXT_MUTED` — use `" • "` between timestamp and source tag |
| Source tag `[app]` | `STATUS_GREEN` |
| Source tag `[flutter]` | `STATUS_INDIGO` |
| Source tag `[watcher]` | `STATUS_BLUE` |
| Source tag `[daemon]` | `STATUS_YELLOW` |
| Source tag `[error]` | `STATUS_RED` |
| Message text | `TEXT_PRIMARY` (Info/default) |
| Error message | `STATUS_RED` or `LOG_ERROR_MSG` |
| Warning message | `STATUS_YELLOW` |
| Debug message | `TEXT_MUTED` |

**Blinking cursor:**
- At the end of the last log line (or on a new line after the last entry)
- Small block character `"█"` in `ACCENT` with `Modifier::SLOW_BLINK`
- Only shown when auto-scroll is active (following live content)

**Empty state:**
- "Not Connected" text in `TEXT_MUTED` + `BOLD`
- "Press + to start a new session" in `TEXT_MUTED`
- Centered vertically in the glass container

**No matches state:**
- "No logs match current filter" in `STATUS_YELLOW` + `ITALIC`
- "Press Ctrl+f to reset filters" in `TEXT_MUTED`

#### Implementation Notes

**Metadata bar rendering:**

The metadata bar must be rendered INSIDE the block's inner area but BEFORE the log entries. Approach:

```rust
// After rendering the block:
let inner = block.inner(area);

// Render metadata bar in the first line of inner
let meta_area = Rect::new(inner.x, inner.y, inner.width, 1);
render_metadata_bar(meta_area, buf);

// Log content starts 1 line below metadata bar
let content_area = Rect::new(inner.x, inner.y + 1, inner.width, inner.height.saturating_sub(1));
// ... render log entries into content_area ...
```

**Separator line (optional):**

If desired, a thin horizontal line can be rendered between the metadata bar and log content:

```rust
let separator = "─".repeat(inner.width as usize);
buf.set_string(inner.x, inner.y + 1, &separator, Style::default().fg(palette::BORDER_DIM));
// content_area then starts at inner.y + 2, inner.height - 2
```

Recommendation: skip the separator for now — the color contrast between metadata bar text and log content provides enough visual distinction.

**LogViewState adjustment:**

The `update_content_size()` and visible_lines calculation must account for the 1 line consumed by the metadata bar:

```rust
// Before
let visible_lines = inner.height as usize;

// After
let visible_lines = inner.height.saturating_sub(1) as usize; // -1 for metadata bar
```

**Source tag color mapping update:**

The current `LogSource → Color` mapping is:
- App → Magenta, Daemon → Yellow, Flutter → Blue, FlutterError → Red, Watcher → Cyan

Update to use the new semantic colors:
- App → `STATUS_GREEN` (was Magenta — the design reference uses green for app)
- Daemon → `STATUS_YELLOW` (unchanged)
- Flutter → `STATUS_INDIGO` (was Blue — now indigo to match design)
- FlutterError → `STATUS_RED` (unchanged)
- Watcher → `STATUS_BLUE` (was Cyan — now sky blue to match design)

This is a deliberate visual change to match the design reference.

**Scrollbar styling:**

Update scrollbar symbols to use theme-appropriate styling. The scrollbar currently overlays the right border — this behavior is acceptable.

### Acceptance Criteria

1. Log view renders as a glass container (`BorderType::Rounded`, `CARD_BG` bg, `BORDER_DIM` border)
2. Top metadata bar shows "TERMINAL LOGS" label and "LIVE FEED" badge
3. Log entries use themed source tag colors matching the design reference
4. Bullet separator `" • "` appears between timestamp and source tag
5. Blinking cursor appears at the end of content when auto-scroll is active
6. Empty state and no-matches state use themed styles
7. All existing features preserved:
   - Vertical scrolling (virtualized)
   - Horizontal scrolling
   - Search highlighting
   - Stack trace collapse/expand
   - Link highlight mode
   - Filter state display in title area (move to metadata bar or keep)
8. `cargo check -p fdemon-tui` passes
9. `cargo clippy -p fdemon-tui` passes

### Testing

- Verify log entries render with correct source tag colors
- Verify metadata bar appears and is properly positioned
- Verify scrolling still works (visible_lines adjusted for metadata bar)
- Verify search highlighting still works
- Test empty state rendering
- Test with very small terminal (metadata bar + 1 line of content minimum)

### Notes

- **This is the most complex task in Phase 2**. The LogView widget is ~1000 lines with many interlocking features. Work incrementally — first change the container styling, then add the metadata bar, then update entry styling, then add the cursor.
- **Filter/search indicators**: Currently shown in the block title. Move to the metadata bar (e.g., "TERMINAL LOGS • Filtered: Error" or "TERMINAL LOGS • Search: 3/10"). Or keep filter info in a subtitle line if space allows.
- **Blinking cursor**: `Modifier::SLOW_BLINK` support varies by terminal. Some terminals ignore it. This is acceptable — it's a polish feature.
- **Source tag color change**: Changing App from Magenta to Green and Flutter from Blue to Indigo is an intentional design change, not a regression. The new colors match the design reference.
- **The bottom metadata bar is NOT in this task** — that's Task 04 (merge status bar). This task handles the top metadata bar only.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Redesigned glass container with rounded borders, added top metadata bar with "TERMINAL LOGS" label and "LIVE FEED" badge, updated log entry styling with bullet separator, changed source tag colors (App→Green, Flutter→Indigo, Watcher→Blue), added blinking cursor for auto-scroll mode, updated empty/no-matches states with themed styles, adjusted visible_lines calculation to account for metadata bar |
| `crates/fdemon-tui/src/widgets/log_view/tests.rs` | Updated test expectation for search highlights (level indicator icon removed in redesign) |

### Notable Decisions/Tradeoffs

1. **Level indicator icon removed**: The redesign replaces the level icon (✗, ⚠, •, ·) with a bullet separator between timestamp and source tag. This matches the design specification and simplifies the visual hierarchy.
2. **Metadata bar includes filter/search indicators**: Filter and search status are now shown inline with "TERMINAL LOGS" label using bullet separator (e.g., "TERMINAL LOGS • Error • Search: 3/10"), providing better visibility than the previous title-based approach.
3. **Blinking cursor conditional**: Cursor only appears when `state.auto_scroll` is true, indicating live feed mode. This provides visual feedback that new content is being tracked.
4. **Visible_lines adjustment**: Content area height is reduced by 1 to account for the metadata bar, ensuring scroll calculations remain accurate.
5. **Source tag color changes**: Intentional design change - App (Magenta→Green), Flutter (Blue→Indigo), Watcher (Cyan→Blue) to match the Cyber-Glass design reference.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-tui` - Passed
- `cargo test -p fdemon-tui` - 473/474 tests passed (1 pre-existing header test failure unrelated to this task)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed
- Snapshot tests updated and accepted for visual changes (rounded borders, metadata bar)

### Risks/Limitations

1. **Terminal support for SLOW_BLINK**: Some terminals may not render the blinking cursor animation. This is acceptable as it's a visual enhancement, not a functional requirement.
2. **One header test failure**: `widgets::header::tests::test_header_with_keybindings` fails, but this is out of scope for this task (header.rs modifications are handled by other tasks per task specification).
3. **Visual regression intentional**: The removal of level indicator icons and change to rounded borders are intentional design changes, not bugs.