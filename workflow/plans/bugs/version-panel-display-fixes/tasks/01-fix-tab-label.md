## Task: Fix SDK Info Tab Label Disappearing When Unfocused

**Objective**: Make the "SDK Info" label always visible in the left pane, styled differently based on focus state — matching the "Installed Versions" header behavior.

**Depends on**: None

### Scope

- `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs`: Modify `SdkInfoPane::render()` to always render the label

### Details

**Current behavior** (`sdk_info.rs:155-184`):
- When `focused == true`: renders "SDK Info" label at row 0 in `ACCENT + BOLD`, then content in reduced area
- When `focused == false`: renders content directly into full area — **no label at all**

**Desired behavior** (matching `VersionListPane::render_list_header()` at `version_list.rs:111-139`):
- Always render "SDK Info" label at row 0
- When focused: style `ACCENT + BOLD`
- When unfocused: style `TEXT_SECONDARY`
- Always render underline separator (`─`) on row 1 in `BORDER_DIM`
- Content area always starts at row 2

**Implementation:**

```rust
// In SdkInfoPane::render() — replace the entire focused/unfocused branching

/// Height of the section header ("SDK Info") + underline separator.
///
/// Derived from: 1 title row + 1 separator row = 2 rows.
const HEADER_HEIGHT: u16 = 2;

fn render(self, area: Rect, buf: &mut Buffer) {
    // Always render header (label + underline) regardless of focus
    self.render_header(area, buf);

    // Content area starts below header
    let content_area = if area.height > HEADER_HEIGHT {
        Rect::new(area.x, area.y + HEADER_HEIGHT, area.width, area.height - HEADER_HEIGHT)
    } else {
        return; // Not enough space for content
    };

    match &self.state.resolved_sdk {
        Some(sdk) => self.render_sdk_details(sdk, content_area, buf),
        None => self.render_no_sdk(content_area, buf),
    }
}

fn render_header(&self, area: Rect, buf: &mut Buffer) {
    if area.height < 1 { return; }

    let title_style = if self.focused {
        Style::default().fg(palette::ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette::TEXT_SECONDARY)
    };

    let label = Line::from(vec![
        Span::raw("  "),
        Span::styled("SDK Info", title_style),
    ]);
    Paragraph::new(label).render(Rect::new(area.x, area.y, area.width, 1), buf);

    // Underline separator
    if area.height >= 2 {
        let sep = "\u{2500}".repeat(area.width as usize);
        buf.set_string(area.x, area.y + 1, &sep, Style::default().fg(palette::BORDER_DIM));
    }
}
```

### Acceptance Criteria

1. "SDK Info" label is visible when `focused == true` (styled `ACCENT + BOLD`)
2. "SDK Info" label is visible when `focused == false` (styled `TEXT_SECONDARY`)
3. Underline separator renders below the label in both states
4. Content area is reduced by header height in both states (consistent positioning)
5. No panic on tiny areas (area.height < 2)
6. Existing tests pass; version field content is not affected

### Testing

```rust
#[test]
fn test_sdk_info_pane_unfocused_shows_label() {
    let state = make_state_with_sdk();
    let pane = SdkInfoPane::new(&state, false); // unfocused
    let area = Rect::new(0, 0, 40, 15);
    let mut buf = Buffer::empty(area);
    pane.render(area, &mut buf);
    let content: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(content.contains("SDK Info"), "unfocused pane should still show label");
}

#[test]
fn test_sdk_info_pane_label_has_underline_separator() {
    let state = make_state_with_sdk();
    let pane = SdkInfoPane::new(&state, true);
    let area = Rect::new(0, 0, 40, 15);
    let mut buf = Buffer::empty(area);
    pane.render(area, &mut buf);
    // Check row 1 contains separator character
    let row1: String = (0..area.width)
        .map(|x| buf.cell((x, 1)).map(|c| c.symbol().to_string()).unwrap_or_default())
        .collect();
    assert!(row1.contains("─"), "should have underline separator below label");
}
```

### Notes

- This is the simplest fix and unblocks task 02 (which adjusts the layout constants with the new header height accounted for).
- The header adds 2 rows of overhead vs the current focused-only 1-row overhead. Task 02 will adjust `VERTICAL_SDK_INFO_HEIGHT` accordingly.
- Mirror the exact pattern from `VersionListPane::render_list_header()` for consistency.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs` | Added `HEADER_HEIGHT` constant; added `render_header()` method; replaced focused/unfocused branching in `Widget::render()` to always call `render_header()`; added two new tests |

### Notable Decisions/Tradeoffs

1. **Header height changed from 1 to 2**: The old focused-only path consumed 1 row; the new path always consumes 2 rows (label + separator). This matches the `VersionListPane` pattern exactly. Task 02 will adjust `VERTICAL_SDK_INFO_HEIGHT` to account for this extra row.
2. **`render_header()` mirrors `render_list_header()` exactly**: Same structure, same palette constants, same separator character — ensures visual consistency between the two panes.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-tui` - Passed
- `cargo test -p fdemon-tui` - Passed (861 tests, 0 failed)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed

### Risks/Limitations

1. **Content area shrinks by 1 extra row**: Because the header now always occupies 2 rows instead of the previous 1-row focused path, the content area is 1 row shorter in the focused state. Task 02 should adjust the overall pane height constant to compensate.
