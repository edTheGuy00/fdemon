## Task: Implement Narrow Layout Vertical Split for Network Tab

**Objective**: Replace the full-width detail overlay on narrow terminals with a vertical split (table top, details bottom), matching the Inspector tab's behavior. This improves UX by keeping both the request list and details visible simultaneously.

**Depends on**: 04 (truncate fix — affects table rendering), 05 (color consistency — affects both table and details)
**Severity**: MAJOR (UX)
**Review ref**: REVIEW.md Issue #7

### Scope

- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs`: Replace `render_narrow_detail` with vertical split layout
- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs`: Update layout routing logic
- `crates/fdemon-tui/src/widgets/devtools/network/tests.rs`: Update/add narrow layout tests

### Current Behavior

In `network/mod.rs` (line ~98-108):

```rust
if area.width >= WIDE_THRESHOLD && has_selection {
    // Wide: horizontal split — table (55%) | details (45%)
    self.render_wide_layout(usable, buf, &filtered);
} else if has_selection && area.width < WIDE_THRESHOLD {
    // Narrow with selection: show details full-width (OVERLAY)
    self.render_narrow_detail(usable, buf);
} else {
    // No selection: full-width table
    self.render_table_only(usable, buf, &filtered);
}
```

On narrow terminals (< `WIDE_THRESHOLD` columns), selecting a request **replaces the table entirely** with a full-width detail view. The user must press Esc to return.

### Target Behavior (Inspector Pattern)

The Inspector tab (inspector/mod.rs line ~141-168) always shows both panels:
- **Wide terminals**: horizontal split (tree left, layout right)
- **Narrow terminals**: vertical split (tree top 50%, layout bottom 50%)

The Network tab should follow the same pattern:
- **Wide terminals** (>= `WIDE_THRESHOLD`): horizontal split — table left, details right (current behavior, keep)
- **Narrow terminals** (< `WIDE_THRESHOLD`) **with selection**: vertical split — table top, details bottom
- **No selection**: full-width table (current behavior, keep)

### Fix

Replace the layout routing in `network/mod.rs`:

```rust
if has_selection {
    if area.width >= WIDE_THRESHOLD {
        // Wide: horizontal split — table | details
        self.render_wide_layout(usable, buf, &filtered);
    } else {
        // Narrow: vertical split — table top | details bottom
        self.render_narrow_split(usable, buf, &filtered);
    }
} else {
    // No selection: full-width table
    self.render_table_only(usable, buf, &filtered);
}
```

Implement `render_narrow_split`:

```rust
fn render_narrow_split(&self, area: Rect, buf: &mut Buffer, filtered: &[&HttpProfileEntry]) {
    let chunks = Layout::vertical([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(area);

    // Top: request table (compact, fewer columns if needed)
    self.render_table_panel(chunks[0], buf, filtered);

    // Bottom: request details
    if let Some(entry) = self.network_state.selected_entry() {
        let detail_widget = RequestDetails::new(/* ... */);
        detail_widget.render(chunks[1], buf);
    }
}
```

The table in the top half may need to show fewer columns (e.g., just Method, URI, Status) since the height is reduced. Consider using the same column-hiding logic that `render_wide_layout` uses for narrow widths.

### Remove `render_narrow_detail`

Delete the `render_narrow_detail` method entirely since it's no longer needed. Also remove any `Esc` key handling that was specific to dismissing the narrow overlay (if separate from the general deselect behavior).

### Keybinding Impact

With a vertical split, the user no longer needs `Enter` to view details or `Esc` to dismiss them — selecting a row automatically shows details in the bottom panel (same as wide mode). This simplifies the interaction model.

Review `handler/keys.rs` to ensure `Enter` and `Esc` still work sensibly:
- `Enter` on a row: could still toggle selection (select/deselect)
- `Esc`: deselect and collapse the detail panel

### Tests

- Snapshot test: narrow terminal with selection shows vertical split
- Snapshot test: wide terminal with selection shows horizontal split
- Snapshot test: no selection shows full-width table (both narrow and wide)
- Verify table renders correctly in reduced height (top half)

### Verification

```bash
cargo test -p fdemon-tui -- network
cargo test -p fdemon-tui -- narrow
cargo clippy -p fdemon-tui
```
