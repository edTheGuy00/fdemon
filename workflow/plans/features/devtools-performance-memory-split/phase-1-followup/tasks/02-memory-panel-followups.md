## Task: Memory panel UX + formatting follow-ups

**Objective:** Resolve every memory-widget-tree finding from the Phase 1 review in a single coherent edit. Add a disconnected-state render path mirroring the Performance panel (M1, m12), restore comma-separated instance counts in the allocation table (M5, m1), replace the manual `Rect` arithmetic with a `Layout::vertical` split (m3), annotate the dead-code fields with their Phase 2 rationale (m2), and expand the tab-bar test to cover all four panels (m5).

**Depends on:** None (Wave 1)

**Agent:** implementor

**Estimated Time:** 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — pass `vm_connected` / `connection_status` into `MemoryPanel::new` instead of discarding it (M1, m12), and expand `test_tab_bar_shows_all_panels` to assert all four panel labels (m5).
- `crates/fdemon-tui/src/widgets/devtools/memory/mod.rs` — extend `MemoryPanel::new` to accept `vm_connected` + `connection_status`, add a `render_disconnected` helper that mirrors `widgets/devtools/performance/mod.rs:131` (M1); rewrite the stale `format_number` comment as a proper doc-comment (m1); restore comma-separated number formatting for instance counts (M5); add Phase-2 rationale comments next to `#[allow(dead_code)]` (m2); replace the manual two-Rect arithmetic in `render_impl` (lines 158–165) with a `Layout::vertical([Constraint::Length(chart_height), Constraint::Min(0)])` split (m3).
- `crates/fdemon-tui/src/widgets/devtools/memory/table.rs` — call the new comma-separated formatter for the instances column (M5).
- `crates/fdemon-tui/src/widgets/devtools/memory/tests.rs` — add `memory_panel_renders_disconnected_state_when_vm_unavailable` (M1) and `format_number_produces_comma_separated_instance_counts` (M5).

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — `render_disconnected` is the reference pattern for the Memory equivalent.
- `crates/fdemon-app/src/state.rs` — `ConnectionStatus` enum that feeds the disconnected-state message.

### Background

The Phase 1 review surfaced six findings inside the memory widget tree, all in the same files. Bundling them keeps the diff coherent and avoids T04's later doc-cleanup task re-touching the same spots.

- **M1 / m12** — Performance, Inspector, and Network panels all guard `!vm_connected` and render a tailored "VM not connected" view; Memory does not. The reviewer noted that `widgets/devtools/mod.rs:162` already captures `(mem, _vm_connected)` from the session arm but discards the connection flag.
- **M5 / m1** — `format_number` in `widgets/devtools/memory/mod.rs:54-65` collapses large counts to K/M/G ("1.2K") — useful for byte sizes (already handled separately by `MemoryUsage::format_bytes`) but reduces precision in the allocation table's instances column where small leak deltas matter. The accompanying header comment "(no longer needed here, but kept for format_number)" is also stale.
- **m2** — `MemoryPanel::focused` and `chart_focused` carry `#[allow(dead_code)]` annotations but no inline justification.
- **m3** — `render_impl` at memory/mod.rs:158–165 manually computes `table_area.y = area.y + chart_height`, violating `docs/CODE_STANDARDS.md` Principle 2 ("all content must fit within the allocated area — include every element in the layout system").
- **m5** — `test_tab_bar_shows_all_panels` in `widgets/devtools/mod.rs:523-527` only asserts the Inspector and Performance labels are present and contains a negative assertion for "Layout" (a name that was never used). Memory and Network labels are absent.

### Details

#### 1. `widgets/devtools/mod.rs` — pass `vm_connected` + `connection_status` into `MemoryPanel`

Locate the `DevToolsPanel::Memory` arm of `render_impl` (around line 162). It currently looks like:

```rust
let (mem, _vm_connected) = match self.session {
    Some(handle) => (&handle.session.memory, /* connection flag */),
    None => /* fallback */,
};
let memory_panel = MemoryPanel::new(mem, /* focused */ true);
```

Change to capture `connection_status` from the session's daemon and pass it into a widened constructor (see step 2 for the constructor):

```rust
let (mem, vm_connected, connection_status) = match self.session {
    Some(handle) => (
        &handle.session.memory,
        /* derive vm_connected from handle.daemon or matching path used by Performance arm */,
        /* derive connection_status similarly */,
    ),
    None => (/* fallback memory */, false, ConnectionStatus::default()),
};
let memory_panel = MemoryPanel::new(mem, /* focused */ true, vm_connected, connection_status);
```

Use the exact extraction pattern from the `Performance` arm (lines ~145–157 — same file) as a template. Do not re-derive the connection status with new logic; copy what Performance does.

#### 2. `widgets/devtools/memory/mod.rs` — widen `MemoryPanel::new` and add `render_disconnected`

Current `MemoryPanel` struct (lines ~85–105) and constructor: the struct has fields `mem`, `focused`, `chart_focused` and the constructor `MemoryPanel::new(mem: &MemoryState, focused: bool)`.

**Required edits:**

1. Add two new fields:

   ```rust
   /// Whether the underlying Dart VM Service is connected. Drives the
   /// disconnected-state render path (mirrors `PerformancePanel`).
   vm_connected: bool,
   /// Connection status string used in the disconnected-state body.
   connection_status: ConnectionStatus,
   ```

   Match whatever exact types the Performance panel uses (consult `widgets/devtools/performance/mod.rs`).

2. Update the constructor signature:

   ```rust
   pub fn new(
       mem: &'a MemoryState,
       focused: bool,
       vm_connected: bool,
       connection_status: ConnectionStatus,
   ) -> Self
   ```

3. Add Phase-2 rationale comments to the existing `#[allow(dead_code)]` fields (m2):

   ```rust
   #[allow(dead_code)] // Phase 2: drives the outer panel border colour when focus tracking lands.
   focused: bool,
   // ...
   #[allow(dead_code)] // Phase 2: drives the chart-section border colour.
   chart_focused: bool,
   ```

4. In `render_impl`, add the disconnected guard immediately before the current "no memory data" path:

   ```rust
   fn render_impl(&self, area: Rect, buf: &mut Buffer) {
       if !self.vm_connected || !self.mem.monitoring_active {
           self.render_disconnected(area, buf);
           return;
       }
       // ...existing render body...
   }
   ```

   The `render_disconnected` helper should mirror `widgets/devtools/performance/mod.rs:131-208`'s tailored message: VM connection state on line 1, "Memory monitoring will start when DevTools connects to the running Flutter app." on line 2, and the `connection_status`-derived hint on line 3. Use `ratatui::Paragraph` centred in `area` (look at the Performance helper for the exact wording style).

5. Replace the manual `Rect` arithmetic at lines 158–165 (m3):

   ```rust
   // Before
   let chart_area = Rect { x: area.x, y: area.y, width: area.width, height: chart_height };
   let table_area = Rect { x: area.x, y: area.y + chart_height, width: area.width, height: table_height };

   // After
   use ratatui::layout::{Constraint, Layout};
   let chunks = Layout::vertical([
       Constraint::Length(chart_height),
       Constraint::Min(0),
   ])
   .split(area);
   let chart_area = chunks[0];
   let table_area = chunks[1];
   ```

   This complies with `docs/CODE_STANDARDS.md` Principle 2 (every visible element belongs to the `Layout`, never positioned outside it). The `Min(0)` absorber gracefully clips if `chart_height` ever exceeds `area.height`.

6. Restore comma-separated number formatting (M5, m1). Remove the stale comment header at line 54 and either:
   - Rename the existing K/M/G `format_number` to `format_compact_count` and keep it for any future use, **and** add a new `format_count_with_commas(n: u64) -> String` that produces "12,345"; OR
   - Replace `format_number` entirely if no other call site needs the K/M/G version.

   Audit call sites first — `table.rs:213` is the known one. If `format_number` is **only** used by the instances column, replace it in place with the comma formatter. Add a proper doc comment:

   ```rust
   /// Format a count with comma separators for the thousands group.
   ///
   /// Used by the allocation table's instances column where exact counts
   /// matter — small leak deltas (12,345 → 12,398) are lost under K/M/G.
   /// Byte-size columns continue to use [`MemoryUsage::format_bytes`].
   fn format_count_with_commas(n: u64) -> String {
       // straightforward implementation — iterate digits in reverse, insert
       // commas every three places. No external crate required.
   }
   ```

   A simple implementation:

   ```rust
   fn format_count_with_commas(n: u64) -> String {
       let s = n.to_string();
       let bytes = s.as_bytes();
       let mut out = String::with_capacity(s.len() + s.len() / 3);
       for (i, &b) in bytes.iter().enumerate() {
           if i > 0 && (bytes.len() - i) % 3 == 0 {
               out.push(',');
           }
           out.push(b as char);
       }
       out
   }
   ```

7. Update `table.rs` call site (M5) — change from `format_number(instances)` to `format_count_with_commas(instances)` (or whatever final name the function ends up with).

#### 3. `widgets/devtools/mod.rs` — expand `test_tab_bar_shows_all_panels` (m5)

Current test at line 523-527:

```rust
fn test_tab_bar_shows_all_panels() {
    // ...
    assert!(text.contains("Inspector"), "Expected Inspector tab");
    assert!(text.contains("Performance"), "Expected Performance tab");
    assert!(
        !text.contains("Layout"),
        "Layout tab should not appear; got: {text:?}"
    );
}
```

Replace the body with assertions for all four panel labels and drop the obsolete negative "Layout" check:

```rust
fn test_tab_bar_shows_all_panels() {
    // ...render setup unchanged...
    assert!(text.contains("Inspector"), "Expected Inspector tab; got: {text:?}");
    assert!(text.contains("Performance"), "Expected Performance tab; got: {text:?}");
    assert!(text.contains("Memory"), "Expected Memory tab; got: {text:?}");
    assert!(text.contains("Network"), "Expected Network tab; got: {text:?}");
}
```

#### 4. `widgets/devtools/memory/tests.rs` — new regression tests

**(a) `memory_panel_renders_disconnected_state_when_vm_unavailable` (M1):**

Construct a `MemoryState::default()` and a `MemoryPanel::new(state, true, false /* vm_connected */, ConnectionStatus::Disconnected)`. Render into a `Buffer` sized 80×10. Assert the rendered text contains a recognisable disconnected-state phrase used by the Performance panel (e.g. "VM Service not connected" or whatever phrasing the existing helper uses) and does **not** contain "Allocations by class" (the live-state table header).

**(b) `format_count_with_commas_produces_comma_separated_output` (M5):**

```rust
assert_eq!(format_count_with_commas(0), "0");
assert_eq!(format_count_with_commas(999), "999");
assert_eq!(format_count_with_commas(1_000), "1,000");
assert_eq!(format_count_with_commas(12_345), "12,345");
assert_eq!(format_count_with_commas(1_234_567), "1,234,567");
assert_eq!(format_count_with_commas(u64::MAX), /* 18,446,744,073,709,551,615 */);
```

The `u64::MAX` case ensures the formatter doesn't trip on the max value.

#### 5. Quality gate

Run the verification suite. See `docs/DEVELOPMENT.md`. All four steps must pass.

### Acceptance Criteria

- [ ] `cargo check`, `cargo test`, `cargo clippy` all green.
- [ ] Memory panel renders a clear disconnected-state view when `vm_connected == false` or `monitoring_active == false`.
- [ ] Allocation table renders instance counts using comma separators ("12,345"), not K/M/G ("12.3K"), for all values up to `u64::MAX`.
- [ ] `MemoryState`-side `render_impl` uses `Layout::vertical` for the chart/table split — no manual `Rect { y: area.y + chart_height, .. }` arithmetic remains.
- [ ] `#[allow(dead_code)]` annotations on `MemoryPanel.focused` and `MemoryPanel.chart_focused` carry a `// Phase 2: ...` rationale comment.
- [ ] `test_tab_bar_shows_all_panels` asserts the presence of all four panel labels: `Inspector`, `Performance`, `Memory`, `Network`.
- [ ] `MemoryPanel::new` signature is widened to accept `vm_connected` and `connection_status` (call site in `widgets/devtools/mod.rs` updated accordingly).
- [ ] The stale "no longer needed here, but kept for format_number" comment block in `memory/mod.rs:54` is removed; the new formatter has a real doc comment.

### Module Structure

No new modules. All edits within existing files. If `format_count_with_commas` ends up shared with other widgets later, consider promoting it to `widgets/devtools/format.rs` in a future cleanup — out of scope for this task.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/memory/mod.rs` | Added `vm_connected` and `connection_status` fields; widened `new()` constructor; added `render_disconnected()` helper mirroring Performance panel; replaced manual `Rect` arithmetic with `Layout::vertical`; renamed `format_number` to `format_count_with_commas` with proper doc comment; added Phase-2 rationale comments to `#[allow(dead_code)]` fields; updated imports |
| `crates/fdemon-tui/src/widgets/devtools/memory/table.rs` | Updated call site from `format_number` to `format_count_with_commas` |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Updated `DevToolsPanel::Memory` arm to capture and pass `vm_connected` and `connection_status` to `MemoryPanel::new`; expanded `test_tab_bar_shows_all_panels` to assert all four panel labels (Memory, Network) and removed obsolete negative "Layout" check |
| `crates/fdemon-tui/src/widgets/devtools/memory/tests.rs` | Added `VmConnectionStatus` import; updated all existing `MemoryPanel::new` calls to widened 4-arg signature; set `monitoring_active = true` on states that test connected-path rendering; added `memory_panel_renders_disconnected_state_when_vm_unavailable` test; added `format_count_with_commas_produces_comma_separated_output` test |

### Notable Decisions/Tradeoffs

1. **Disconnected guard before compact summary**: The `render_disconnected` guard fires before the height-based compact summary path (`area.height < MIN_CHART_HEIGHT`). This matches the Performance panel behavior exactly — a disconnected state is shown regardless of terminal size.

2. **Existing tests updated with `monitoring_active = true`**: Tests that previously rendered chart/table content relied on the panel proceeding without checking `monitoring_active`. Those tests now explicitly set the flag to maintain test intent (connected + monitoring active = chart renders).

3. **Clippy `is_multiple_of` suggestion applied**: The `(bytes.len() - i) % 3 == 0` pattern was replaced with `(bytes.len() - i).is_multiple_of(3)` per clippy's suggestion, keeping the implementation idiomatic.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-tui` - Passed (1113 tests)
- `cargo test --workspace` - Passed (all suites)
- `cargo clippy --workspace` - Passed (no warnings)
- `cargo fmt --all` - Applied and verified clean

### Risks/Limitations

1. **No risks identified**: All changes are contained within the memory widget tree and devtools mod with no impact on other subsystems.
