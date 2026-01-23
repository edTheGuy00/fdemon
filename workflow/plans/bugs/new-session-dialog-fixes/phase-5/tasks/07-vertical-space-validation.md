## Task: Validate Vertical Space Budget at Minimum Height

**Objective**: Verify that the compact mode dialog is usable at the minimum terminal height (20 lines) with borders enabled.

**Priority**: Critical

**Depends on**: Tasks 1-6 (code changes should be complete first)

### Scope

- Manual testing at terminal height of exactly 20 lines
- Document the vertical space breakdown
- Adjust `MIN_VERTICAL_HEIGHT` if needed

### Problem Analysis

The review identified that adding 4 lines of borders (2 per section) with `MIN_VERTICAL_HEIGHT: 20` leaves only 16 lines for content. No analysis confirmed this is sufficient.

**Current border usage:**
- Target Selector: Top border (1) + Bottom border (1) = 2 lines
- Launch Context: Top border (1) + Bottom border (1) = 2 lines
- **Total border overhead: 4 lines**

**Available for content: 20 - 4 = 16 lines**

### Verification Steps

1. **Set terminal to exact dimensions:**
   ```bash
   # macOS Terminal: Window > Window Size > 80 columns, 20 rows
   # iTerm2: Preferences > Profiles > Window > Columns: 80, Rows: 20
   # Or use: printf '\e[8;20;80t'
   ```

2. **Launch application and open new session dialog:**
   ```bash
   cargo run
   # Press 'n' to open new session dialog
   ```

3. **Document what's visible:**
   - [ ] Tab bar (Connected / Bootable tabs)
   - [ ] Device list with at least 2-3 items visible
   - [ ] All config fields (Name, Mode, Flavor, Dart Defines)
   - [ ] Launch button
   - [ ] Cancel button or dismiss instruction

4. **Test interactions:**
   - [ ] Can navigate device list with arrow keys
   - [ ] Can switch between tabs
   - [ ] Can scroll if list exceeds visible area
   - [ ] Can access all form fields

### Expected Space Breakdown

```
Line 1:  Target Selector top border (─────────────────)
Line 2:  Tab bar [Connected] [Bootable]
Line 3:  Device 1
Line 4:  Device 2
Line 5:  Device 3 (or scrollable indicator)
Line 6:  Target Selector bottom border (─────────────────)
Line 7:  Launch Context top border (─────────────────)
Line 8:  Config name field
Line 9:  Mode selection
Line 10: Flavor field
Line 11: Dart Defines field
Line 12: [Empty or additional field]
Line 13: Launch Context bottom border (─────────────────)
Line 14-20: Status bar, help text, or additional content
```

### If Content Overflows

**Option A: Increase MIN_VERTICAL_HEIGHT**

In `src/tui/widgets/new_session_dialog/mod.rs` (or wherever defined):
```rust
const MIN_VERTICAL_HEIGHT: u16 = 22;  // Was 20, increased to fit borders
```

**Option B: Reduce border usage in compact mode**

Only show borders at larger sizes:
```rust
let show_borders = area.height >= 25;  // Only borders at comfortable sizes
```

**Option C: Remove title from borders in compact mode**

```rust
// Instead of "─── Target ───"
// Just use "─────────────"
```

### Acceptance Criteria

1. At terminal height 20, all essential elements are visible
2. At terminal height 20, all interactions work (navigation, selection, launch)
3. Space breakdown is documented
4. If adjustments needed, they are implemented and tested
5. No content is cut off or inaccessible

### Documentation Update

After validation, add to `docs/ARCHITECTURE.md` or create `docs/UI_LAYOUT.md`:

```markdown
## New Session Dialog Layout

### Minimum Terminal Size
- Width: 80 columns
- Height: 20 rows

### Vertical Space Budget (Compact Mode)
| Component | Lines |
|-----------|-------|
| Target Selector border (top) | 1 |
| Tab bar | 1 |
| Device list (min visible) | 3 |
| Target Selector border (bottom) | 1 |
| Launch Context border (top) | 1 |
| Config fields | 4 |
| Launch Context border (bottom) | 1 |
| Footer/help | 2 |
| **Total** | **14** |
| **Available** | **20** |
| **Buffer** | **6** |
```

### Notes

- This is a manual testing task, not a code change task
- Results inform whether code changes are needed
- If MIN_VERTICAL_HEIGHT must increase, document the rationale

---

## Completion Summary

**Status:** Not Started

**Testing Results:**
- Terminal dimensions tested:
- All elements visible: Yes/No
- Interactions work: Yes/No
- Adjustments needed: Yes/No

**Space Breakdown:**
(Fill in after testing)

**Files Modified:**
(If adjustments were needed)
