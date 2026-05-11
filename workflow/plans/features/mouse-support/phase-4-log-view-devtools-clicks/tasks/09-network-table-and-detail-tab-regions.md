## Task: Network Table Row + Detail-Tab Region Recording

**Objective**: Inside the Network panel, register one click region per visible request row in `request_table.rs::render_rows` (`Emit(Message::NetworkSelectRequest { index: Some(entry_idx) })`), and one click region per detail-sub-tab label in `request_details.rs::render_tab_bar` (`Emit(Message::NetworkSwitchDetailTab(tab))`).

**Depends on**: Task 02 (sister-function scaffold for `NetworkMonitor`)

**Estimated Time**: 1.25 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs`: Replace the no-op delegation in `render_with_regions` with a real body that mirrors the existing `Widget::render` layout dispatch (wide / narrow / table-only) and threads `Option<&mut MouseCtx<'_>>` into both the table and detail surfaces.
- `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs`: Promote `render_rows` to accept `Option<&mut MouseCtx<'_>>` (or introduce a sister function). Register one click region per visible row inside the existing `for (row_idx, entry_idx) in (start..end).enumerate()` loop.
- `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs`: Promote `render_tab_bar` to accept `Option<&mut MouseCtx<'_>>`. Register one click region per detail-tab label inside the existing `for (tab, label) in &tabs` loop.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/message.rs::Message::NetworkSelectRequest`, `Message::NetworkSwitchDetailTab`, `NetworkDetailTab`
- `crates/fdemon-app/src/mouse_regions.rs::MouseAction::emit`, `MouseRect`
- `crates/fdemon-tui/src/render/mod.rs::MouseCtx`

### Details

#### Request-table row regions

In `request_table.rs::render_rows`:

```rust
fn render_rows(&self, area: Rect, buf: &mut Buffer, mut ctx: Option<&mut MouseCtx<'_>>) {
    if area.height == 0 {
        return;
    }
    let visible_rows = area.height as usize;
    let start = self.scroll_offset;
    let end = (start + visible_rows).min(self.entries.len());

    for (row_idx, entry_idx) in (start..end).enumerate() {
        let entry = self.entries[entry_idx];
        let y = area.y + row_idx as u16;

        // ... existing background + cell rendering ...

        if let Some(c) = ctx.as_deref_mut() {
            let rect = MouseRect::new(area.x, y, area.width, 1);
            if rect.width > 0 && rect.height > 0 {
                c.click(
                    rect,
                    MouseAction::emit(Message::NetworkSelectRequest {
                        index: Some(entry_idx),
                    }),
                );
            }
        }
    }
}
```

The two columns headers row (`render_column_headers`) and the recording-status header row are not clickable in v1. (Phase 5 may make `[● REC]` a click-to-toggle-recording target, but that would conflict with the existing keyboard `R` shortcut — defer.)

#### Detail-tab regions

In `request_details.rs::render_tab_bar`:

```rust
fn render_tab_bar(&self, area: Rect, buf: &mut Buffer, mut ctx: Option<&mut MouseCtx<'_>>) {
    let tabs = [
        (NetworkDetailTab::General, "[g] General"),
        (NetworkDetailTab::Headers, "[h] Headers"),
        (NetworkDetailTab::RequestBody, "[q] Request"),
        (NetworkDetailTab::ResponseBody, "[s] Response"),
        (NetworkDetailTab::Timing, "[t] Timing"),
    ];

    let mut x = area.x;
    for (tab, label) in &tabs {
        let padded = format!(" {} ", label);
        let needed_width = padded.len() as u16;

        if x >= area.right() {
            break;
        }

        // ... existing style + buf.set_string ...

        if let Some(c) = ctx.as_deref_mut() {
            let render_w = needed_width.min(area.right().saturating_sub(x));
            if render_w > 0 {
                let rect = MouseRect::new(x, area.y, render_w, 1);
                c.click(
                    rect,
                    MouseAction::emit(Message::NetworkSwitchDetailTab(*tab)),
                );
            }
        }

        x += needed_width;
    }
}
```

#### Layout dispatch in `render_with_regions`

```rust
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    widget: NetworkMonitor<'_>,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    // Re-create the existing Widget::render dispatch, threading ctx into
    // the table and details branches.
    if !widget.vm_connected {
        widget.render_disconnected(area, buf);
        return;
    }
    if widget.network_state.extensions_available == Some(false) {
        widget.render_unavailable(area, buf);
        return;
    }

    let usable = Rect { height: area.height.saturating_sub(1), ..area };
    if usable.height < MIN_USABLE_HEIGHT || usable.width < MIN_USABLE_WIDTH {
        widget.render_too_small(usable, buf);
        return;
    }

    let content_area = if widget.network_state.filter_input_active {
        widget.render_filter_input_bar(Rect { height: 1, ..usable }, buf);
        Rect {
            y: usable.y + 1,
            height: usable.height.saturating_sub(1),
            ..usable
        }
    } else {
        usable
    };

    if content_area.height < MIN_USABLE_HEIGHT || content_area.width < MIN_USABLE_WIDTH {
        return;
    }

    let filtered = widget.network_state.filtered_entries();
    let has_selection = widget.network_state.selected_index.is_some();

    if has_selection {
        if area.width >= WIDE_THRESHOLD {
            widget.render_wide_layout_with_regions(content_area, buf, &filtered, ctx);
        } else {
            widget.render_narrow_split_with_regions(content_area, buf, &filtered, ctx);
        }
    } else {
        widget.render_table_only_with_regions(content_area, buf, &filtered, ctx);
    }
}
```

The `render_wide_layout_with_regions`, `render_narrow_split_with_regions`, `render_table_only_with_regions` are new sibling methods that mirror the existing `render_wide_layout` / `render_narrow_split` / `render_table_only` but pass `ctx` into the table render and (for split layouts) into `RequestDetails::render_with_regions`.

`RequestDetails::render_with_regions` is the sister function for the detail panel; it threads ctx into `render_tab_bar` and renders the active tab content (which has no clicks in v1).

### Acceptance Criteria

1. With `Some(ctx)` passed in and 10 visible rows, the registry contains 10 row regions in the table area.
2. Each row region carries `MouseAction::Emit(Message::NetworkSelectRequest { index: Some(i) })` where `i` is the absolute index into `filtered` (i.e., `entry_idx`, which equals `start + row_idx` after applying `scroll_offset`).
3. With a request selected (split layout), the detail panel's tab bar registers 5 click regions, one per `NetworkDetailTab` variant.
4. Each detail-tab region carries `MouseAction::Emit(Message::NetworkSwitchDetailTab(tab))`.
5. Detail-tab region width matches the padded `format!(" {label} ")` cells (`needed_width` cols, 1 row).
6. With no request selected (table-only layout), no detail-tab regions are registered.
7. With `network.filter_input_active == true`, the filter-input bar still renders but click regions are still registered. (The dispatcher in Task 05 suppresses clicks; the regions exist but are unreachable.)
8. With `vm_connected == false` or `extensions_available == Some(false)`, no regions are registered (the disconnected / unavailable views render).
9. Calling the existing `Widget::render` (without ctx) registers no regions.
10. `cargo test --workspace`, `cargo fmt`, `cargo clippy -- -D warnings`, `cargo check` pass. ≥ 3 new unit tests.

### Testing

```rust
#[test]
fn network_table_records_one_region_per_visible_row() {
    use fdemon_app::message::Message;
    use fdemon_app::{MouseRegions, MouseAction};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use crate::render::MouseCtx;

    let entries = make_entries(10);
    let refs: Vec<&fdemon_core::network::HttpProfileEntry> = entries.iter().collect();
    let table = RequestTable::new(&refs, None, 0, true, "");

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, 12); // 12 - 2 (header rows) = 10 data rows
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        table.render_with_regions(area, &mut buf, Some(&mut ctx));
    }

    let click_indices: Vec<usize> = regions
        .iter()
        .filter_map(|e| match e.on_left.as_ref().and_then(|a| a.as_emit()) {
            Some(Message::NetworkSelectRequest { index: Some(i) }) => Some(*i),
            _ => None,
        })
        .collect();
    assert_eq!(click_indices, (0..10).collect::<Vec<_>>());
}

#[test]
fn detail_tab_bar_records_five_regions() {
    use fdemon_app::message::Message;
    use fdemon_app::{MouseRegions};
    use crate::render::MouseCtx;

    let entry = make_entry("id", "GET", Some(200));
    let details = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        details.render_with_regions(area, &mut buf, Some(&mut ctx));
    }

    let tab_clicks: Vec<NetworkDetailTab> = regions
        .iter()
        .filter_map(|e| match e.on_left.as_ref().and_then(|a| a.as_emit()) {
            Some(Message::NetworkSwitchDetailTab(t)) => Some(*t),
            _ => None,
        })
        .collect();
    assert_eq!(
        tab_clicks,
        vec![
            NetworkDetailTab::General,
            NetworkDetailTab::Headers,
            NetworkDetailTab::RequestBody,
            NetworkDetailTab::ResponseBody,
            NetworkDetailTab::Timing,
        ]
    );
}

#[test]
fn network_disconnected_records_no_regions() {
    use fdemon_app::{MouseRegions};
    use crate::render::MouseCtx;

    let network_state = fdemon_app::session::NetworkState::default();
    let widget = NetworkMonitor::new(&network_state, /*vm_connected=*/ false, &VmConnectionStatus::Disconnected);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, widget, Some(&mut ctx));
    }

    assert_eq!(regions.iter().count(), 0);
}
```

### Notes

- **Why one wide row region instead of per-column.** Per-column rects (Status / Method / Duration / Size / Type / URI) would let future enhancements bind column-specific clicks (e.g., click Status to filter by code). v1 keeps it as one wide rect — consistent with the Inspector tree and Performance frame chart.
- **Why the header row + column-header row are not clickable.** The recording indicator (`● REC` / `○ PAUSED`) and column headers are informational. Clicking `● REC` to toggle recording would conflict with the existing `R` keyboard binding semantically — let the Phase 5 dialog/overlay pass evaluate whether that conflict is real.
- **Filter-input bar regions.** When `filter_input_active = true`, the top row shows a filter-input bar. Clicks there could plausibly cancel input or move the cursor; v1 doesn't bind any. The dispatcher (Task 05) gates clicks while filter is active anyway, so even rows registered below would be unreachable.
- **Detail-tab regions overlap with body content?** No — `render_tab_bar` runs on the first row of the detail panel; the body content starts on the second row. The tab regions are 1 row tall and don't overflow.
- **`network.selected_index` is the index into filtered_entries**, not the raw entries vec. Click regions emit the same index. `handle_network_select_request` already operates in the filtered domain — no conversion needed.
- **Detail-tab labels and keyboard shortcuts.** The labels `[g] General` / `[h] Headers` / `[q] Request` / `[s] Response` / `[t] Timing` already mirror keyboard shortcuts via `keys.rs`. Clicking them produces the same `NetworkSwitchDetailTab` message that the keyboard handler emits.
- **Don't register a region on the detail-panel border / dividers.** The detail panel has a 1-cell `Borders::LEFT` block in wide layout and an implicit divider in narrow layout. Clicks on the border should be silent.
- **Existing `Widget::render` for `RequestTable` and `NetworkMonitor`** must continue to work without ctx. Adapt by adding sister `render_with_regions` methods rather than mutating `Widget::render`.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/network/mod.rs` | Replaced stub `render_with_regions` free fn with full dispatch body; added `render_wide_layout_with_regions`, `render_narrow_split_with_regions`, `render_table_only_with_regions` methods; existing layout methods now delegate to the `_with_regions` variants with `None` ctx. |
| `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs` | Added `render_with_regions` public method and private `render_impl`; promoted `render_rows` to accept `Option<&mut MouseCtx<'_>>`; registers one `NetworkSelectRequest { index: Some(entry_idx) }` region per visible data row. |
| `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs` | Added `render_with_regions` public method and private `render_impl`; promoted `render_tab_bar` to accept `Option<&mut MouseCtx<'_>>`; registers one `NetworkSwitchDetailTab(tab)` region per sub-tab label with correct padded widths. |
| `crates/fdemon-tui/src/widgets/devtools/network/tests.rs` | Added `region_tests` submodule with 7 new unit tests verifying: row count, absolute scroll-offset indices, 5 detail tab regions in order, disconnected/unavailable early-return registers no regions, None ctx path, and tab region widths. |

### Notable Decisions/Tradeoffs

1. **`Widget::render` delegates to `render_impl`**: Rather than duplicating the header+column render logic, `Widget::render` calls the private `render_impl(area, buf, None)`. This avoids the pattern where `Widget::render` called `render_with_regions` (which would make `render_with_regions` the canonical path rather than a sister function).

2. **Clippy `needless_option_as_deref` vs `option_as_ref_deref`**: When `ctx` must be reborrowed for a first call and then passed by move for a second call, calling `ctx.as_deref_mut()` for the first use and `ctx` (moved) for the second is the pattern that satisfies both lints. The lints conflict when both reborrows are used.

3. **`render_table_only` delegation**: `render_table_only` and `render_narrow_split` now delegate to their `_with_regions` counterparts with `None` ctx, eliminating code duplication.

4. **No region on detail-panel border**: The `Borders::LEFT` block in wide layout is rendered before `render_with_regions` is called on the inner area. The border is excluded from the detail inner area so no click region is registered there.

### Testing Performed

- `cargo check -p fdemon-tui` - Passed
- `cargo test --workspace --lib` - Passed (920 fdemon-tui tests, all green)
- `cargo fmt --all` - Passed
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **`needless_option_as_deref` is version-sensitive**: The clippy lint behavior observed required using `as_deref_mut()` for intermediate reborrows and moving `ctx` for the final consumer. Future clippy updates may produce different recommendations.
2. **No filter-bar click regions**: Per the spec, the filter input bar does not register click regions in v1. The dispatcher (Task 05) suppresses clicks during filter input anyway.
