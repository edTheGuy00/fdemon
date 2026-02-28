## Task: Implement Height-Based Compact Decision in `render_horizontal()`

**Objective**: In the horizontal layout path, check the actual height of the content pane and pass `compact(true)` to LaunchContext when the pane is too short for expanded fields. This fixes the "wide-but-short terminal" problem where expanded fields overflow the dialog.

**Depends on**: 01-threshold-constants, 02-render-panes-compact-arg

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`: Modify `render_horizontal()` (lines 439-493)

### Details

**Current code flow in `render_horizontal()`:**
```
1. centered_rect(area) → dialog_area
2. Draw dialog border → inner area
3. Layout::vertical split inner → [header(3), sep(1), content(Min(10)), sep(1), footer(1)]
4. render_panes(chunks[2], buf)       ← chunks[2] is the content area
5. Render header, footer, separators
```

The content area `chunks[2]` is the area passed to `render_panes()`, which splits it 40/60 horizontally. The right 60% pane receives LaunchContext. The height of this pane equals `chunks[2].height` (same for both left and right panes in a horizontal split).

**New logic** — replace step 4 with:
```rust
// Determine if LaunchContext needs compact mode based on available height
let launch_compact = chunks[2].height < MIN_EXPANDED_LAUNCH_HEIGHT;
self.render_panes(chunks[2], buf, launch_compact);
```

That's it. The height check is a single comparison against the threshold constant from task 01.

**Why this works:**
- `chunks[2]` is the `Min(10)` slot from the vertical layout. Its actual height = `inner.height - 3 (header) - 1 (sep) - 1 (sep) - 1 (footer) = inner.height - 6`.
- `inner.height = dialog_area.height - 2` (for the dialog border).
- `dialog_area.height = area.height * 70%` (from `centered_rect`).
- So for a terminal height of 30: `dialog_area.height = 21`, `inner.height = 19`, `chunks[2].height = 13`. Since `13 < 28`, compact mode activates. Correct.
- For a terminal height of 50: `dialog_area.height = 35`, `inner.height = 33`, `chunks[2].height = 27`. Since `27 < 28`, compact still activates. Also reasonable — 27 rows is borderline for 29-row expanded content.
- For a terminal height of 55: `dialog_area.height = 38`, `inner.height = 36`, `chunks[2].height = 30`. Since `30 >= 28`, expanded mode. Correct.

**Interaction with existing TooSmall guard:**
- `TooSmall` activates when `area.height < 20` or `area.width < 40`. This is checked in `layout_mode()` before `render_horizontal()` is ever called. The new height check is a second layer of protection for cases where the terminal is tall enough for horizontal layout but too short for expanded fields.

### Acceptance Criteria

1. In horizontal layout mode, LaunchContext renders in compact mode when `chunks[2].height < MIN_EXPANDED_LAUNCH_HEIGHT`
2. In horizontal layout mode, LaunchContext renders in expanded mode when `chunks[2].height >= MIN_EXPANDED_LAUNCH_HEIGHT`
3. The TargetSelector (left pane) always renders in full mode in horizontal layout (unchanged)
4. A terminal at 100x25 shows compact LaunchContext in horizontal layout
5. A terminal at 100x50 shows expanded LaunchContext in horizontal layout
6. `cargo check -p fdemon-tui` passes
7. `cargo test -p fdemon-tui` passes — all existing tests remain green

### Testing

New tests will be added in task 05. For this task, verify manually or with existing tests:
- `cargo test -p fdemon-tui` must pass (existing `test_dialog_renders` etc.)
- Existing `layout_mode` tests are not affected (layout mode is unchanged — this is a rendering decision within horizontal mode, not a layout mode change)

### Notes

- The `centered_rect()` function uses 70% height for horizontal mode. This means the content pane height is roughly `terminal_height * 0.70 - 8` (8 rows for dialog border, header, separators, footer). The threshold of 28 means expanded mode activates at roughly `terminal_height >= 52`. This is reasonable — most full-screen terminals are 40+ rows.
- If we want expanded mode to kick in at shorter terminals, we could reduce the `MIN_EXPANDED_LAUNCH_HEIGHT` threshold or adjust `centered_rect()` percentages. But changing percentages is out of scope for Phase 1.
- The compact LaunchContext in horizontal mode will have the "Launch Context" titled border (from `render_compact()`), which looks slightly different from the borderless expanded mode. This is intentional and consistent with how compact mode works in vertical layout.
