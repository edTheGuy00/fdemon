# Task 03 — Compact-Mode Glyph for Inactive Refreshing Tabs (F4)

**Agent:** implementor
**Phase:** 1
**Depends on:** none
**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`

---

## Goal

Fix Minor finding F4 from PR #37's Copilot review: in compact mode, the `↻` refresh
glyph appears only when the *refreshing tab is the active tab*. Full mode (`TabBar`)
shows the glyph per-tab regardless of active state, matching the PR description.
Compact mode should match.

## Context

Current code at
`crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs:233-252`:

```rust
let connected_label = if connected_active {
    if self.state.refreshing {
        format!("[1 Connected {}]", self.icons.refresh())
    } else {
        "[1 Connected]".to_string()
    }
} else {
    "1 Connected".to_string()  // <-- no glyph if inactive, even if refreshing
};

let bootable_label = if bootable_active {
    if self.state.bootable_refreshing {
        format!("[2 Bootable {}]", self.icons.refresh())
    } else {
        "[2 Bootable]".to_string()
    }
} else {
    "2 Bootable".to_string()  // <-- same problem
};
```

The nesting puts the `refreshing` check *inside* the `*_active` branch, so the
inactive case never appends the glyph. Full-mode `TabBar` (in `tab_bar.rs`) computes
the glyph independently from active state.

Reviewer's suggested replacement (in the PR comment) inverts the nesting: compute
the base label first (with active-tab brackets or bare), then conditionally append
the glyph.

## Steps

1. Open `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` and
   locate `render_tabs_compact` (around line 220).

2. Replace the label-construction block (currently lines 233-252) with the
   refactored version:

   ```rust
   // Build the base label (active-tab brackets vs bare), then conditionally
   // append the refresh glyph for any tab whose refresh is in flight. This
   // mirrors `TabBar`'s per-tab semantics in full mode.
   let connected_base = if connected_active {
       "[1 Connected]"
   } else {
       "1 Connected"
   };
   let connected_label = if self.state.refreshing {
       format!("{} {}", connected_base, self.icons.refresh())
   } else {
       connected_base.to_string()
   };

   let bootable_base = if bootable_active {
       "[2 Bootable]"
   } else {
       "2 Bootable"
   };
   let bootable_label = if self.state.bootable_refreshing {
       format!("{} {}", bootable_base, self.icons.refresh())
   } else {
       bootable_base.to_string()
   };
   ```

3. **Add a render test** in the `#[cfg(test)] mod tests` block at the bottom of
   `target_selector.rs`. Mirror the existing test
   `test_target_selector_compact_renders_refreshing_glyph` (added in
   device-cache-followup task 04) but for the *inactive* tab case:

   - **`test_target_selector_compact_renders_refreshing_glyph_on_inactive_tab`**:
     - Build a `TargetSelectorState` with `bootable_refreshing = true` and
       `active_tab = TargetTab::Connected` (so Bootable is the inactive,
       refreshing tab).
     - Render `TargetSelector::new(...).compact(true)` into a small buffer.
     - Convert the buffer to a string (use the existing buffer-to-string helper if
       present in the test module, e.g. `buffer_to_string` or inline iteration).
     - Assert the rendered output contains `IconSet::default().refresh()` (which
       resolves to `"\u{21bb}"`, i.e. `"↻"`).
     - Use a diagnostic assertion message: `"expected refresh glyph on inactive
       Bootable tab in compact mode, got: {rendered}"`.
     - Symmetric variant: `connected_active = false` (i.e. set
       `active_tab = TargetTab::Bootable`) with `refreshing = true` — assert the
       glyph appears next to the inactive Connected label too. Combine into one
       test or split into two; one combined test is fine if the assertion is clear.

4. **Do not change** `render_full` or `TabBar`. Full-mode rendering is correct.

5. **Do not change** `target_selector.rs:35-56` (`new`, `icons`, `compact`
   builders). Task 04 handles `IconSet` threading separately.

6. Run verification:
   - `cargo fmt --all`
   - `cargo check -p fdemon-tui`
   - `cargo test -p fdemon-tui --lib`
   - `cargo clippy -p fdemon-tui --lib -- -D warnings`

## Acceptance Criteria

- [ ] `render_tabs_compact` computes base labels first (active-tab brackets vs
      bare) then conditionally appends the refresh glyph based on each tab's
      `*_refreshing` flag, regardless of active state.
- [ ] When the active tab is Connected and `bootable_refreshing == true`, the
      rendered compact output contains the refresh glyph next to the Bootable
      label.
- [ ] Symmetric: when the active tab is Bootable and `refreshing == true`, the
      rendered compact output contains the glyph next to the Connected label.
- [ ] Existing test `test_target_selector_compact_renders_refreshing_glyph`
      (active-tab case) still passes unchanged — the new label-construction logic
      preserves the active-tab + refreshing glyph behavior.
- [ ] New test `test_target_selector_compact_renders_refreshing_glyph_on_inactive_tab`
      passes.
- [ ] `cargo test -p fdemon-tui --lib` passes (no regressions).
- [ ] `cargo clippy -p fdemon-tui --lib -- -D warnings` clean.

## Out of Scope

- Modifying `TabBar` or `render_full`. Full-mode rendering is correct.
- Threading `IconSet` from `NewSessionDialog` to `TargetSelector` (owned by
  task 04).
- Adding new public APIs to `TargetSelector` or new builder methods. The fix is
  internal to `render_tabs_compact`.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a969ce4f369774621

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | Refactored `render_tabs_compact` label construction to separate base-label (brackets vs bare) from glyph appending; added `test_target_selector_compact_renders_refreshing_glyph_on_inactive_tab` test |

### Notable Decisions/Tradeoffs

1. **Two-block test structure**: The new test uses two inner scopes (Case 1 and Case 2) within a single `#[test]` function as suggested in the task, providing clear diagnostic messages for each scenario while keeping the test count tidy.
2. **No change to active-tab behavior**: The refactored label construction preserves the `[brackets]` for the active tab and bare text for the inactive tab, just reorders the nesting so the glyph check is outer.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-tui` - Passed
- `cargo test -p fdemon-tui --lib` - Passed (877 tests)
- `cargo clippy -p fdemon-tui --lib -- -D warnings` - Passed (clean)
- `cargo test -p fdemon-tui --lib "target_selector"` - Passed (44 tests, including new inactive-tab test)

### Risks/Limitations

1. **None**: The change is purely internal to `render_tabs_compact`; no public API was modified and no layer boundaries were touched.
