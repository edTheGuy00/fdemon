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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/network/mod.rs` | Replaced `render_narrow_detail` with `render_narrow_split`; updated layout routing; updated module/struct doc comments |
| `crates/fdemon-tui/src/widgets/devtools/network/tests.rs` | Renamed `test_narrow_terminal_with_selection_shows_detail_only` to `test_narrow_terminal_with_selection_shows_vertical_split`; added `test_narrow_terminal_with_selection_shows_both_panels`; added `test_narrow_terminal_just_below_threshold_uses_vertical_split` |

### Notable Decisions/Tradeoffs

1. **Reused `render_table_only` in narrow split**: The task referenced a `render_table_panel` method name that does not exist. The existing `render_table_only` method is equivalent and was used directly. No column-hiding logic was added for the reduced height, as the `RequestTable` widget already handles small areas gracefully via its `height < 2` guard.

2. **No border between narrow split panels**: The wide layout adds a `Borders::LEFT` border on the detail side. The narrow split uses no border (same pattern as the Inspector tab's vertical split), keeping the implementation simple.

3. **`filtered` slice passed through to narrow split**: The signature of `render_narrow_split` mirrors `render_wide_layout` by taking `filtered: &[&HttpProfileEntry]`, avoiding a re-computation of filtered entries. This is consistent with the existing wide layout helper.

### Testing Performed

- `cargo test -p fdemon-tui -- network` — Passed (112 tests)
- `cargo test -p fdemon-tui -- narrow` — Passed (6 tests, including 3 new narrow-specific tests)
- `cargo clippy -p fdemon-tui` — Passed (no warnings)
- `cargo fmt --all -- --check` — Passed
- `cargo test --lib --workspace` — Passed (716 unit tests)

### Risks/Limitations

1. **No column-hiding on narrow split**: The table in the top 50% shows all columns. On very narrow terminals (< ~50 cols), columns may appear truncated. This is the same behavior as the existing full-width table on narrow terminals without selection, so no regression was introduced.

2. **E2E test failures are pre-existing**: 25 e2e integration tests under `tests/e2e/` fail in the workspace run; these require a running Flutter environment and are unrelated to this change.
