## Task: Fix Info Banner Height Allocation

**Objective**: Fix the info banners on USER and VSCODE tabs that render as empty bordered boxes. The height is allocated as 3 lines but needs 4 (2 border lines + 2 content lines).

**Depends on**: None

**Severity**: Critical (user-reported bug)

### Scope

- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`: Fix height allocation and content offsets at 2 locations
- `crates/fdemon-tui/src/widgets/settings_panel/tests.rs`: Add/update tests for info banner content

### Details

#### Root Cause

Info banners use `Block::default().borders(Borders::ALL)` which consumes 2 lines (top + bottom border). The inner area needs 2 lines (icon+title line + subtitle line). Total required: **4 lines**.

Currently allocated: **3 lines**, giving inner height = 1. The guard `if inner.height < 2 { return; }` always triggers, so banners render as empty bordered boxes.

The task spec (05-redesign-special-views.md line 271) explicitly states: "The current info banner is 4 lines (border top + 2 content + border bottom). The redesigned version is 4 lines too."

#### Fix Location 1: User Preferences Tab (line 507)

```rust
// BEFORE (line 507):
let info_area = Rect::new(area.x, area.y, area.width, 3);

// AFTER:
let info_area = Rect::new(area.x, area.y, area.width, 4);
```

Update content area offset (lines 511-515):
```rust
// BEFORE:
let content_area = Rect::new(
    area.x,
    area.y + 3,
    area.width,
    area.height.saturating_sub(3),
);

// AFTER:
let content_area = Rect::new(
    area.x,
    area.y + 4,
    area.width,
    area.height.saturating_sub(4),
);
```

#### Fix Location 2: VSCode Tab (line 910)

```rust
// BEFORE (line 910):
let info_area = Rect::new(area.x, area.y, area.width, 3);

// AFTER:
let info_area = Rect::new(area.x, area.y, area.width, 4);
```

Update content area offset (lines 914-918):
```rust
// BEFORE:
let content_area = Rect::new(
    area.x,
    area.y + 3,
    area.width,
    area.height.saturating_sub(3),
);

// AFTER:
let content_area = Rect::new(
    area.x,
    area.y + 4,
    area.width,
    area.height.saturating_sub(4),
);
```

### Acceptance Criteria

1. USER tab info banner displays bordered box with icon + "Local Settings" title and subtitle text
2. VSCODE tab info banner displays bordered box with icon + "VSCode Launch Configurations" title and subtitle text
3. Content below each info banner starts at the correct Y offset (no gap or overlap)
4. No visual regression on small terminals (the `if inner.height < 2` guard should now pass for height-4 allocation)

### Testing

Add tests that verify info banner content renders (not just the border):

```rust
#[test]
fn test_user_prefs_info_banner_shows_content() {
    // Render USER tab with sufficient height
    // Verify "Local Settings" text appears in the banner area
}

#[test]
fn test_vscode_info_banner_shows_content() {
    // Render VSCODE tab with sufficient height
    // Verify "VSCode" text appears in the banner area
}
```

### Notes

- This is a simple off-by-one fix at 2 locations with corresponding offset updates
- The `render_user_prefs_info()` and `render_vscode_info()` functions themselves are correct — they properly render 2 lines of content. Only the allocation in the parent function is wrong.
- Verify that both `render_user_prefs_info` (line 569) and `render_vscode_info` (line 985) still work correctly after the height change

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` | Fixed info banner height allocation from 3→4 lines at 2 locations (User prefs tab line 507, VSCode tab line 910) and updated content area offsets from 3→4 (lines 513-515, 916-918) |
| `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` | Added 2 tests: `test_user_prefs_info_banner_shows_content()` and `test_vscode_info_banner_shows_content()` to verify info banner content renders correctly |

### Notable Decisions/Tradeoffs

1. **Height Allocation Fix**: Changed from 3 to 4 lines to accommodate 2 border lines + 2 content lines. This matches the original design spec which explicitly states info banners should be 4 lines total.
2. **Content Area Offset**: Updated content area Y offset and height calculation to account for the new 4-line info banner, ensuring content starts immediately below the banner with no gap or overlap.
3. **Test Coverage**: Added tests that verify the actual content text appears in the rendered buffer, not just that the banner renders without errors. This prevents regression to the empty-box bug.

### Testing Performed

- `cargo test -p fdemon-tui info_banner_shows_content` - PASSED (2 new tests)
- `cargo test -p fdemon-tui` - PASSED (all 443 tests passed, 7 doc tests passed)
- Both new tests verify that "Local Settings" and "VSCode" text appear in the rendered banner content

### Risks/Limitations

1. **Concurrent Changes**: The working tree also contains changes from other agents (empty state functions at lines 817, 1054, 1140 and truncate_str in styles.rs). These changes are outside the scope of this task and were not made by this implementation. Only the info banner height fixes and tests were implemented as specified.
2. **Visual Testing**: Automated tests verify text content appears in buffer, but manual visual inspection would confirm the exact layout and appearance. The fix is mathematically correct (4 lines = 2 border + 2 content) and matches the design spec.
