## Task: Add Compact Parameter to `render_panes()`

**Objective**: Modify `render_panes()` to accept a `compact` flag and pass it to both `TargetSelector` and `LaunchContextWithDevice`, enabling the horizontal layout path to dynamically choose compact mode based on available height.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`: Modify `render_panes()` signature and body (lines 309-340)

### Details

**Current signature** (line 309):
```rust
fn render_panes(&self, area: Rect, buf: &mut Buffer)
```

**New signature:**
```rust
fn render_panes(&self, area: Rect, buf: &mut Buffer, launch_compact: bool)
```

The parameter is named `launch_compact` (not just `compact`) because the TargetSelector in horizontal mode should generally stay in full mode — it has the full left pane width and the 40% split gives it adequate height. The LaunchContext on the right side is the one that overflows when the pane is too short for expanded fields.

**Changes to `render_panes()` body:**

Current LaunchContext construction (lines 333-339):
```rust
let launch_context = LaunchContextWithDevice::new(
    &self.state.launch_context,
    launch_focused,
    has_device,
    self.icons,
);
launch_context.render(chunks[2], buf);
```

New:
```rust
let launch_context = LaunchContextWithDevice::new(
    &self.state.launch_context,
    launch_focused,
    has_device,
    self.icons,
)
.compact(launch_compact);
launch_context.render(chunks[2], buf);
```

**Update the call site in `render_horizontal()`** (currently around line 483):
```rust
// Before:
self.render_panes(chunks[2], buf);

// After (temporary - task 03 will add height logic):
self.render_panes(chunks[2], buf, false);
```

This is a mechanical refactor — the behavior should be identical after this task (always passing `false` from the only call site).

### Acceptance Criteria

1. `render_panes()` accepts a `launch_compact: bool` parameter
2. The parameter is forwarded to `LaunchContextWithDevice` via `.compact(launch_compact)`
3. The call site in `render_horizontal()` passes `false` (preserving current behavior)
4. `TargetSelector` construction in `render_panes()` is NOT changed (stays without `.compact()`)
5. `cargo check -p fdemon-tui` passes
6. `cargo test -p fdemon-tui` passes — all existing tests remain green
7. No behavioral change — this is a pure refactor

### Testing

No new tests needed — this is a signature change that preserves existing behavior. Existing tests validate that the dialog renders correctly.

### Notes

- `render_panes()` is only called from `render_horizontal()` (one call site), so this is a low-risk change.
- We intentionally do NOT add a `target_compact` parameter. In horizontal mode, the TargetSelector gets 40% of a >= 70-column-wide pane, which gives it at least 28 columns and the full pane height. Compact mode for the target selector is only needed in vertical layout, which constructs the widget directly (not through `render_panes()`).
- Future: If we ever need compact TargetSelector in horizontal mode, we can add a second parameter then.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Added `launch_compact: bool` parameter to `render_panes()` signature; chained `.compact(launch_compact)` on `LaunchContextWithDevice` construction; updated call site in `render_horizontal()` to pass `false` |

### Notable Decisions/Tradeoffs

1. **Pure mechanical refactor**: The `TargetSelector` construction was deliberately left unchanged per acceptance criterion 4. Only `LaunchContextWithDevice` receives the `launch_compact` flag, consistent with the task rationale that the right pane is what overflows at reduced heights.

2. **Pre-existing unused constant warnings**: The 4 dead-code warnings for `MIN_EXPANDED_LAUNCH_HEIGHT`, `COMPACT_LAUNCH_HEIGHT_THRESHOLD`, `MIN_EXPANDED_TARGET_HEIGHT`, and `COMPACT_TARGET_HEIGHT_THRESHOLD` were already present before this task. They are intentional stubs for future tasks (task 03+).

### Testing Performed

- `cargo check -p fdemon-tui` - Passed (4 pre-existing dead_code warnings, no errors)
- `cargo test -p fdemon-tui` - Passed (773 unit tests + 7 doc tests, 0 failed)

### Risks/Limitations

1. **No behavioral change**: Confirmed — the only call site passes `false`, so `LaunchContextWithDevice` receives `compact(false)` which is equivalent to its prior default (no `.compact()` call), as the builder method initializes `compact` to `false`.
