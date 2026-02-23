## Task: Implement Wrap Mode Rendering, Line Height Calculation, and Status Indicator

**Objective**: Modify the `LogView` widget to conditionally use ratatui's `Paragraph::wrap()` when wrap mode is enabled, fix the line height calculation for accurate scroll bounds, add a `[wrap]`/`[nowrap]` indicator to the metadata bar, and wire the wrap mode flag through the render path.

**Depends on**: 01-wrap-state

### Scope

- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: Add `wrap_mode` field to `LogView`, conditional render path, line height fix, status indicator
- `crates/fdemon-tui/src/render/mod.rs`: Pass `wrap_mode` from `LogViewState` to `LogView` builder

### Details

#### 1. Add `wrap_mode` field to `LogView` widget

**File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs`

Add `wrap_mode: bool` to the `LogView` struct (around line 68). Default to `false` in `new()`.

Add a builder method:

```rust
pub fn wrap_mode(mut self, enabled: bool) -> Self {
    self.wrap_mode = enabled;
    self
}
```

#### 2. Import `Wrap` from ratatui

Add `Wrap` to the ratatui widgets import (lines 13-22):

```rust
use ratatui::widgets::{
    Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    StatefulWidget, Widget, Wrap,  // ADD Wrap
};
```

#### 3. Conditional rendering path in `render()`

In the `StatefulWidget::render()` implementation, the key change is at lines 1178-1197. When `self.wrap_mode` is `true`:

- **Skip `apply_horizontal_scroll()`** — pass raw lines directly to the paragraph
- **Use `Paragraph::new(lines).wrap(Wrap { trim: false })`** instead of plain `Paragraph`

When `self.wrap_mode` is `false`: keep existing horizontal scroll behavior unchanged.

```rust
// Lines ~1178-1197 — replace the current block:
let final_lines = if self.wrap_mode {
    // Wrap mode: skip horizontal scroll, let ratatui wrap
    all_lines
} else {
    // No-wrap mode: apply horizontal scroll truncation
    let h_offset = state.h_offset;
    let visible_width = content_area.width as usize;
    all_lines
        .into_iter()
        .map(|line| Self::apply_horizontal_scroll(line, h_offset, visible_width))
        .collect()
};

// ... cursor line append ...

if self.wrap_mode {
    Paragraph::new(final_lines)
        .wrap(Wrap { trim: false })
        .render(content_area, buf);
} else {
    // Render log content WITHOUT wrapping (lines are truncated/scrolled)
    Paragraph::new(final_lines).render(content_area, buf);
}
```

#### 4. Fix line height calculation for scroll bounds

This is the most complex part. `calculate_entry_lines()` (lines 598-614) currently assumes each logical line = 1 terminal row. When wrap mode is on, a single logical line may span multiple terminal rows.

**Approach**: When `wrap_mode` is `true`, multiply each logical line by `ceil(line_char_width / visible_width)`. This requires knowing `visible_width` at calculation time.

Add a helper method to `LogView`:

```rust
/// Calculate how many terminal rows a logical line occupies.
/// In wrap mode, long lines wrap to multiple rows.
fn line_display_height(&self, line_char_width: usize, visible_width: usize) -> usize {
    if !self.wrap_mode || visible_width == 0 || line_char_width <= visible_width {
        return 1;
    }
    // Ceiling division
    (line_char_width + visible_width - 1) / visible_width
}
```

**Update `total_lines` calculation** (lines 1036-1039):

The current code sums `calculate_entry_lines()` for each filtered entry. When wrap mode is on, we need a different approach. Since `calculate_entry_lines()` returns the number of *logical* lines per entry (message + stack frames), and each logical line may occupy multiple terminal rows when wrapped, we need to compute the actual terminal row count.

However, computing exact wrapped heights for all entries before rendering is expensive — it requires knowing each line's character width, which isn't available until `format_entry()` is called.

**Pragmatic approach**: Compute `total_lines` after building `all_lines` (the Vec of formatted Lines), since `all_lines` is already built before the scroll state update. But `all_lines` is only the visible window of lines, not all entries.

**Better approach**: In wrap mode, use the `total_lines` from `calculate_entry_lines()` as an approximation. The scroll offset logic already works on logical lines. The `Paragraph::wrap()` handles visual wrapping internally. The scrollbar position may be slightly inaccurate for very long wrapped lines, but this is acceptable since:
- Most log lines are short and fit on one row even when wrapped
- The scrollbar is a position indicator, not a precise pixel-level control
- This matches how other TUI apps (e.g., `less -S` vs `less`) handle wrapping

**Recommended implementation**: Keep `calculate_entry_lines()` returning logical line counts. The existing scroll offset/skip logic (line 1056-1059) works on logical lines and remains correct — each entry still has the same number of logical lines regardless of wrap mode. The `Paragraph::wrap()` handles the visual expansion internally. The only inaccuracy is the scrollbar position, which uses `total_lines / visible_lines` ratio.

If the scrollbar inaccuracy is noticeable in practice, a future refinement can compute wrapped heights per-entry, but this is likely unnecessary for v1.

#### 5. Update `max_line_width` and `update_horizontal_size()` call

When wrap mode is on, `max_line_width` still needs to be computed (for toggling back to nowrap), but `update_horizontal_size()` should still be called. No change needed — the existing code at lines 1167-1175 remains correct.

#### 6. Add `[wrap]` / `[nowrap]` indicator to metadata bar

**File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs`

In `render_metadata_bar()` (lines 649-717), add a wrap mode indicator to the `indicator_parts` vector (around lines 667-692):

```rust
// After filter and search indicators, before joining:
if self.wrap_mode {
    indicator_parts.push("wrap".to_string());
} else {
    indicator_parts.push("nowrap".to_string());
}
```

This produces output like: `TERMINAL LOGS • Errors | wrap` or `TERMINAL LOGS • nowrap`.

#### 7. Wire wrap_mode in render/mod.rs

**File:** `crates/fdemon-tui/src/render/mod.rs`

In the `LogView` builder chain (lines 73-111), add `.wrap_mode()` after the base construction:

```rust
let mut log_view = widgets::LogView::new(&handle.session.logs, icons)
    .filter_state(&handle.session.filter_state)
    .wrap_mode(handle.session.log_view_state.wrap_mode);  // NEW
```

The `wrap_mode` value comes from `handle.session.log_view_state.wrap_mode` (set in Task 01).

### Acceptance Criteria

1. When `wrap_mode` is `true`, long lines wrap at the terminal width without horizontal scrolling
2. When `wrap_mode` is `false`, existing horizontal scroll behavior is preserved exactly
3. `apply_horizontal_scroll()` is NOT called when wrap mode is enabled
4. `Paragraph::new(lines).wrap(Wrap { trim: false })` is used when wrap mode is enabled
5. Metadata bar shows `wrap` or `nowrap` indicator
6. Scroll position and scrollbar remain functional in wrap mode (some imprecision acceptable)
7. `cargo check -p fdemon-tui` passes
8. `cargo clippy -p fdemon-tui -- -D warnings` passes
9. All existing `fdemon-tui` tests pass (`cargo test -p fdemon-tui`)

### Notes

- **Ratatui Wrap semantics**: `Wrap { trim: false }` preserves leading whitespace. `trim: true` would remove leading spaces on continuation lines, which would break indented log messages.
- **Scroll accuracy tradeoff**: In wrap mode, the scroll offset operates on logical lines, not terminal rows. This means scrolling up/down by 1 still moves by 1 logical line (which may be multiple terminal rows if wrapped). This is the same behavior as `less` and most editors, so it's intuitive.
- **Performance**: `apply_horizontal_scroll()` is skipped entirely in wrap mode, so there's no per-line character decomposition overhead. The `Paragraph::wrap()` handles wrapping internally in ratatui's render pass.
- **The `all_lines` vec is still built the same way** — `format_entry()` produces logical lines. The only difference is whether they pass through `apply_horizontal_scroll()` or go directly to a wrapping `Paragraph`.
