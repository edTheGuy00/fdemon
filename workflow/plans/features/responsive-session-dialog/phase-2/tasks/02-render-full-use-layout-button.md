## Task: Update `render_full()` to Use Layout-Managed Button Slot

**Objective**: Replace the manual button `Rect` construction in both `LaunchContext::render()` and `LaunchContextWithDevice::render_full()` with the layout-managed slot `chunks[11]` from `calculate_fields_layout()`. This eliminates the overflow bug where the button could render beyond `area.bottom()`.

**Depends on**: 01-extend-layout-with-button-slot

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs`: Modify `LaunchContext::render()` (lines 852-879) and `LaunchContextWithDevice::render_full()` (lines 929-954)

### Details

There are two render paths that contain the identical manual button placement bug. Both must be fixed.

**Fix 1: `LaunchContext::render()` (lines 852-879)**

Current code (lines 863-869):
```rust
// Calculate launch button area (after dart defines field + spacer)
let button_area = Rect {
    x: area.x + 1,
    y: chunks[9].y + chunks[9].height + 1,
    width: area.width.saturating_sub(2),
    height: 3,
};
```

New code:
```rust
// Button area from layout — inset by 1 on each side for padding
let button_area = Rect {
    x: chunks[11].x + 1,
    width: chunks[11].width.saturating_sub(2),
    ..chunks[11]
};
```

The `x + 1` / `width - 2` inset is preserved to maintain the existing visual padding (the button doesn't touch the left/right edges of the content area). The `y` and `height` now come from the layout system, which guarantees they stay within `area` bounds.

**Fix 2: `LaunchContextWithDevice::render_full()` (lines 929-954)**

Current code (lines 939-945):
```rust
// Calculate launch button area
let button_area = Rect {
    x: area.x + 1,
    y: chunks[9].y + chunks[9].height + 1,
    width: area.width.saturating_sub(2),
    height: 3,
};
```

New code — identical to Fix 1:
```rust
// Button area from layout — inset by 1 on each side for padding
let button_area = Rect {
    x: chunks[11].x + 1,
    width: chunks[11].width.saturating_sub(2),
    ..chunks[11]
};
```

**Why `chunks[11]` is safe**: Ratatui's `Layout::vertical` splits the given `area` into non-overlapping `Rect`s that never exceed `area` bounds. When `area.height` is less than the sum of all `Length` constraints (29 rows), the solver proportionally shrinks slots. The button at `chunks[11]` will either:
- Have `height == 3` (normal case, enough space) — button renders correctly within bounds
- Have `height < 3` (tight space) — button is partially visible but still within bounds
- Have `height == 0` (no space) — button is invisible, which is correct degraded behavior

In all cases, `chunks[11].y + chunks[11].height <= area.y + area.height` is guaranteed by Ratatui.

**Note on `LaunchContext` vs `LaunchContextWithDevice`**: `LaunchContext` is the simpler variant used in contexts without device awareness (it always enables the launch button). `LaunchContextWithDevice` has a `has_device_selected` flag and is the primary widget used in the dialog. Both have the same bug and get the same fix.

### Acceptance Criteria

1. `LaunchContext::render()` uses `chunks[11]` for button placement instead of manual arithmetic
2. `LaunchContextWithDevice::render_full()` uses `chunks[11]` for button placement instead of manual arithmetic
3. The button's horizontal inset (`x + 1`, `width - 2`) is preserved — visual appearance unchanged
4. When `area.height >= 29`, the button renders identically to before (same position and size)
5. When `area.height < 29`, the button stays within `area` bounds (no overflow)
6. No manual `Rect` construction with `chunks[9].y + chunks[9].height + 1` remains in the file
7. `cargo check -p fdemon-tui` passes
8. `cargo test -p fdemon-tui` passes — all existing tests remain green
9. `cargo clippy -p fdemon-tui -- -D warnings` passes

### Testing

Verify with existing tests. The behavioral change is only visible at edge-case sizes (height < 29 in expanded mode), which Phase 1's compact auto-switch normally prevents from occurring. However, the fix is correct even without the compact guard — it's defense-in-depth.

- `cargo test -p fdemon-tui` — existing `launch_context_tests` render at 50x30 which has height >= 29, so the button `Rect` values are identical
- Visual inspection: the button should appear in the same position at normal sizes

Task 03 adds explicit overflow tests.

### Notes

- The comment `// Calculate launch button area (after dart defines field + spacer)` should be updated to reflect the new approach: `// Button area from layout — inset by 1 on each side for padding`
- After this task, the only manual `Rect` construction remaining in the render paths is the horizontal inset (`x + 1`, `width - 2`), which cannot overflow because it only reduces the button's width.
- The `chunks[10]` slot (button spacer, `Length(1)`) is not rendered — it's just an empty spacer row. No code needs to reference it.
- If `chunks[11].height == 0` (due to Ratatui constraint solver collapse), the `LaunchButton` widget will receive a zero-height `Rect` and render nothing. This is safe — `LaunchButton::render()` writes text to `buf` at absolute coordinates within the given `Rect`, and a zero-height Rect means no cells are written.
