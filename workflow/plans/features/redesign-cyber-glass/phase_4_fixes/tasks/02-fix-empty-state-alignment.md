## Task: Fix Empty State Vertical Alignment

**Objective**: Change empty state rendering in Launch and VSCode tabs from vertical centering back to top-aligned with horizontal centering, matching the user's preferred layout.

**Depends on**: None

**Severity**: Critical (user-reported regression)

### Scope

- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`: Fix 3 empty state functions
- `crates/fdemon-tui/src/widgets/settings_panel/tests.rs`: Update alignment tests if any exist

### Details

#### Root Cause

Three empty state functions use vertical centering:
```rust
let start_y = area.top() + area.height.saturating_sub(total_height) / 2;
```

The user reports that the previous behavior (top-aligned, horizontally centered) looked nicer. The current centering was introduced by the Phase 4 task spec, but it's a design regression.

#### Fix: All 3 Functions

Change the vertical positioning from centered to top-aligned with a small top margin (1 line):

```rust
// BEFORE:
let start_y = area.top() + area.height.saturating_sub(total_height) / 2;

// AFTER:
let start_y = area.top() + 1;
```

The horizontal centering (icon box centering + `Alignment::Center` on text) should remain unchanged — it already works correctly.

#### Function 1: `render_launch_empty_state` (line 817)

- **total_height**: 7 (icon box 3 + gap 1 + title 1 + gap 1 + subtitle 1)
- **Icon**: `icons.layers()`
- **Title**: "No launch configurations found"
- **Subtitle**: "Create .fdemon/launch.toml or press 'n' to create one."

#### Function 2: `render_vscode_not_found` (line 1054)

- **total_height**: 8 (icon box 3 + gap 1 + title 1 + gap 1 + subtitle 2)
- **Icon**: `icons.code()`
- **Title**: "No .vscode/launch.json found"
- **Subtitle**: 2 lines of instructions

#### Function 3: `render_vscode_empty` (line 1140)

- **total_height**: 8 (icon box 3 + gap 1 + title 1 + gap 1 + subtitle 2)
- **Icon**: `icons.code()`
- **Title**: "launch.json exists but has no Dart configurations"
- **Subtitle**: 2 lines of instructions

### Acceptance Criteria

1. All 3 empty states render content starting near the top of the content area (1-line top margin)
2. Text remains horizontally centered (icon box centered, text uses `Alignment::Center`)
3. Content is not cut off on small terminals — the existing height guards (`if area.height < total_height + 2`) still apply
4. No visual gaps between icon box, title, and subtitle

### Testing

Update or add tests verifying top-alignment:

```rust
#[test]
fn test_launch_empty_state_top_aligned() {
    // Render launch tab with no configs
    // Verify content starts near area.top(), not vertically centered
}
```

### Notes

- Keep the existing `if area.height < total_height + 2 { ... }` small-terminal fallback as-is
- The small-terminal fallback shows only the title (no icon/subtitle) — this is fine
- All 3 functions share nearly identical structure. A future task could extract a shared helper, but this task only fixes alignment.
