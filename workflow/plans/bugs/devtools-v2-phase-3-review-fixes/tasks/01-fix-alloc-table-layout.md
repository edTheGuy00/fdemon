## Task: Fix allocation table layout threshold

**Objective**: Make the allocation table visible on standard 24-row terminals by fixing the layout arithmetic that prevents it from rendering.

**Depends on**: None

**Source**: Review Critical Issue #1 (Logic & Reasoning Checker, Risks & Tradeoffs Analyzer)

### Scope

- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs`: Lower `MIN_TABLE_HEIGHT` from 3 to 2
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`: Adjust 55/45 split to 50/50
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: Fix footer overlap with memory block border

### Details

#### Root Cause

The allocation table is gated by `show_table = area.height >= MIN_CHART_HEIGHT + MIN_TABLE_HEIGHT` (= `6 + 3 = 9`). On a 24-row terminal with single session:

```
Terminal: 24 rows
  Header: 3 rows → areas.logs = 21 rows
  DevTools sub-tab bar: 3 rows → PerformancePanel area = 18 rows
  55% frame outer: 9 rows, 45% memory outer: 9 rows
  Memory inner (minus Borders::ALL): 7 rows
  7 < 9 → show_table = false
```

The table first appears at terminal height **30** (single session).

#### Fix Strategy (three changes combined)

**1. Lower `MIN_TABLE_HEIGHT` from 3 to 2** (`memory_chart.rs:28`)

The current value of 3 means the table needs space for header (1 row) + separator (1 row) + 1 data row. Lowering to 2 means we accept showing just the header + 1 data row (no separator), which is still useful information.

```rust
// Before
const MIN_TABLE_HEIGHT: u16 = 3;

// After
const MIN_TABLE_HEIGHT: u16 = 2;
```

This changes the threshold from `6 + 3 = 9` to `6 + 2 = 8`.

**2. Adjust split from 55/45 to 50/50** (`mod.rs:152-155`)

A 50/50 split gives the memory section more room. On 18-row panel area: each gets 9 outer rows, which is unchanged — but this helps at larger terminal sizes by giving memory an equal share.

```rust
// Before
let chunks = Layout::vertical([
    Constraint::Percentage(55),
    Constraint::Percentage(45),
])

// After
let chunks = Layout::vertical([
    Constraint::Percentage(50),
    Constraint::Percentage(50),
])
```

**3. Fix footer overlap** (`devtools/mod.rs:261-287`)

The `render_footer` method writes to `area.y + area.height - 1` of the full panel content area (`chunks[1]` from DevToolsView). This row falls inside the memory block's `Borders::ALL` bottom border, overwriting it.

Fix: The performance panel layout should account for 1 row of footer. Subtract 1 from the performance panel area before the 50/50 split, or move footer rendering to be *outside* the bordered blocks.

The recommended approach: In `performance/mod.rs`, reduce the panel area by 1 row to leave room for the DevTools footer, since the footer is rendered by the parent `DevToolsView`:

```rust
// In PerformancePanel::render_content(), account for parent's footer row
let usable_height = total_h.saturating_sub(1); // leave 1 row for DevTools footer
```

Use `usable_height` instead of `total_h` when computing the split.

#### Post-Fix Layout (24-row terminal, single session)

```
Terminal: 24 rows
  Header: 3 rows → areas.logs = 21 rows
  DevTools sub-tab bar: 3 rows → PerformancePanel area = 18 rows
  Usable (minus footer): 17 rows
  50% frame outer: 8 rows, 50% memory outer: 9 rows
  Memory inner (minus Borders::ALL): 7 rows
  show_table = 7 >= 6 + 2 = 8 → still false on 24-row
```

Hmm — even with both changes, 24-row is tight. Let's also consider reducing `MIN_CHART_HEIGHT` from 6 to 5 for the memory chart specifically when the table is being shown. Or alternatively, use `Borders::TOP` instead of `Borders::ALL` on the memory section to save 1 row:

**Alternative 3b: Use `Borders::TOP` on memory section** (`mod.rs`)

Replace `Borders::ALL` with `Borders::TOP` on the memory block. This saves 1 bottom border row (the sides and bottom aren't essential), giving the inner area 8 rows instead of 7.

```
Memory outer: 9 rows
Memory inner (minus Borders::TOP only): 8 rows  (save 1 vs Borders::ALL)
show_table = 8 >= 6 + 2 = 8 → TRUE
```

This is the simplest change that makes it work on 24-row terminals.

**Recommended final combination:**
- Lower `MIN_TABLE_HEIGHT` from 3 to 2
- Change 55/45 split to 50/50
- Use `Borders::TOP` instead of `Borders::ALL` on memory section block
- Keep the title in the top border (ratatui supports `Block::new().borders(Borders::TOP).title(...)`)

### Acceptance Criteria

1. Allocation table is visible on a 24-row terminal (single session) with at least 1 data row
2. Allocation table is visible on a 24-row terminal (multi session, 2+ sessions) — may require compact mode fallback
3. Footer hint text (`[Esc] Logs  [i] Inspector...`) does not overlap the memory block border
4. Frame chart still has sufficient space for useful bar rendering (minimum 5 inner rows)
5. Existing rendering tests pass (update assertions for new layout proportions)

### Testing

Add or update tests that verify the allocation table renders at various terminal heights:

```rust
#[test]
fn test_allocation_table_visible_on_24_row_terminal() {
    // Simulate a 24-row terminal: PerformancePanel receives ~18 rows
    // Memory section inner area should be >= MIN_CHART_HEIGHT + MIN_TABLE_HEIGHT
    // Table should render at least 1 data row
}

#[test]
fn test_allocation_table_visible_on_30_row_terminal() {
    // Verify table renders multiple data rows on larger terminals
}

#[test]
fn test_footer_does_not_overlap_memory_border() {
    // Verify the DevTools footer and memory block bottom border
    // don't occupy the same row
}
```

### Notes

- The `DUAL_SECTION_MIN_HEIGHT` constant (14) should also be reviewed — at 14 rows, each inner section is 5 rows after borders, which is below `MIN_CHART_HEIGHT = 6`. Consider raising to 16 or adjusting the compact threshold logic.
- Test on both single-session (3-row header) and multi-session (5-row header) layouts.
- The `COMPACT_THRESHOLD` (7) is fine as-is — it correctly gates the dual-section path.
