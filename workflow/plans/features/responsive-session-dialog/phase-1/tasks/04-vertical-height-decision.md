## Task: Implement Height-Based Expanded Decision in `render_vertical()`

**Objective**: In the vertical layout path, check the actual height of the LaunchContext area and allow expanded mode when sufficient space is available. Also make the TargetSelector compact/full decision height-aware. This fixes the "narrow-but-tall terminal" problem where compact mode is forced despite ample vertical space.

**Depends on**: 01-threshold-constants

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`: Modify `render_vertical()` (lines 496-574)

### Details

**Current code flow in `render_vertical()`:**
```
1. centered_rect_custom(90, 85, area) → dialog_area
2. Draw dialog border → inner area
3. Layout::vertical split inner → [header(2), sep(1), target(Pct(45)), sep(1), launch(Min(10)), sep(1), footer(1)]
4. TargetSelector::new(...).compact(true).render(chunks[2], buf)     ← always compact
5. LaunchContextWithDevice::new(...).compact(true).render(chunks[4], buf)  ← always compact
6. Render header, footer, separators
```

**New logic for LaunchContext** — replace step 5:
```rust
// Determine if LaunchContext can use expanded mode based on available height
let launch_compact = chunks[4].height < MIN_EXPANDED_LAUNCH_HEIGHT;
let launch_context = LaunchContextWithDevice::new(
    &self.state.launch_context,
    launch_focused,
    has_device,
    self.icons,
)
.compact(launch_compact);
launch_context.render(chunks[4], buf);
```

**New logic for TargetSelector** — replace step 4:
```rust
// Determine if TargetSelector can use full mode based on available height
let target_compact = chunks[2].height < MIN_EXPANDED_TARGET_HEIGHT;
let target_selector = TargetSelector::new(
    &self.state.target_selector,
    self.tool_availability,
    target_focused,
)
.compact(target_compact);
target_selector.render(chunks[2], buf);
```

**Height analysis for vertical layout:**
- `dialog_area.height = area.height * 85%` (from `centered_rect_custom(90, 85, area)`)
- `inner.height = dialog_area.height - 2` (dialog border)
- Layout: `2 + 1 + Pct(45) + 1 + Min(10) + 1 + 1 = 7 fixed rows`
- Remaining for target + launch: `inner.height - 7`
- Target gets: `(inner.height - 7) * 45%`
- Launch gets: remaining after target, minimum 10

**Example calculations:**
- Terminal 50x25 (narrow-but-tall): `dialog.h = 21`, `inner.h = 19`, remaining = 12, target = 5, launch = 7. Launch `7 < 28` → compact. Target `5 < 10` → compact. Same as current behavior.
- Terminal 50x50 (narrow-and-tall): `dialog.h = 42`, `inner.h = 40`, remaining = 33, target = 15, launch = 18. Launch `18 < 28` → compact. Target `15 >= 10` → full. Partial improvement.
- Terminal 50x70 (very tall): `dialog.h = 59`, `inner.h = 57`, remaining = 50, target = 22, launch = 28. Launch `28 >= 28` → expanded! Target `22 >= 10` → full. Full improvement.

The narrow-but-very-tall scenario (e.g., side panel with 70+ rows) now correctly shows expanded LaunchContext.

**Key consideration**: In vertical layout, the Launch Context area (`chunks[4]`) uses `Min(10)` which gets all remaining space after the 45% TargetSelector. For expanded mode to activate (needing 28 rows), the terminal needs to be quite tall in vertical layout. This is expected — vertical layout is for narrow terminals, and only very tall narrow terminals (side panels) benefit from expanded mode.

### Acceptance Criteria

1. In vertical layout, LaunchContext renders expanded when `chunks[4].height >= MIN_EXPANDED_LAUNCH_HEIGHT`
2. In vertical layout, LaunchContext renders compact when `chunks[4].height < MIN_EXPANDED_LAUNCH_HEIGHT`
3. In vertical layout, TargetSelector renders full when `chunks[2].height >= MIN_EXPANDED_TARGET_HEIGHT`
4. In vertical layout, TargetSelector renders compact when `chunks[2].height < MIN_EXPANDED_TARGET_HEIGHT`
5. A terminal at 50x25 shows compact for both (same as current behavior)
6. A terminal at 50x70 shows expanded LaunchContext and full TargetSelector
7. `cargo check -p fdemon-tui` passes
8. `cargo test -p fdemon-tui` passes — all existing tests remain green

### Testing

New tests will be added in task 05. For this task, verify with existing tests:
- `cargo test -p fdemon-tui` must pass
- Existing compact-mode tests (`test_target_selector_compact_*`) should still pass since they use small terminal sizes that will naturally trigger compact mode

### Notes

- The vertical layout footer also has compact vs full variants (`render_footer_compact` vs `render_footer`). These should remain tied to the overall layout mode (Vertical → compact footer), not to the LaunchContext compact decision. The footer shows key hints and is independent of field rendering.
- The header in vertical mode is already compact (2 rows vs 3 rows in horizontal). This should remain unchanged.
- The `Percentage(45)` split for TargetSelector in vertical mode is fixed. The plan's "Future Enhancements" section mentions making this dynamic based on device count, but that's out of scope for Phase 1.
